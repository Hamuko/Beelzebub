use futures::StreamExt;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use log::{debug, error, info, warn, LevelFilter};
use notify::Watcher;
use reqwest::{StatusCode, Url};
use simple_logger::SimpleLogger;

mod config;
mod win;

type ProcessWatchMap = HashMap<u32, Watch>;

struct Watch {
    start: Instant,
    executable: String,
    name: Option<String>,
}

impl Watch {
    fn new(process: win::Process) -> (u32, Self) {
        let name = process.get_display_name();
        (
            process.process_id,
            Self {
                start: Instant::now(),
                executable: process.name,
                name: name,
            },
        )
    }
}

async fn handle_process_start(
    config: &RwLock<config::Config>,
    map: &mut ProcessWatchMap,
    event: win::ProcessStartResult,
) {
    let event = match event {
        Ok(event) => event,
        Err(error) => {
            warn!("Could not process start event: {:?}", error);
            return;
        }
    };

    // Processes with no reported path are probably system stuff and not worth to track.
    let Some(executable_path) = &event.target_instance.executable_path else {
        debug!(
            "Process {} ({}) does not have a path",
            event.target_instance.name, event.target_instance.process_id
        );
        return;
    };

    let path = Path::new(&executable_path);
    let config = config.read().unwrap();
    if !config.is_monitored(path) {
        debug!(
            "Process {} ({}) isn't configured for watching",
            event.target_instance.name, event.target_instance.process_id
        );
        return;
    }

    // TODO: Limit tracking based on parent processes?

    let (pid, watch) = Watch::new(event.target_instance);
    let product_name_display = watch.name.clone();
    info!(
        "Starting watch for {} ({} {})",
        product_name_display.unwrap_or("?".to_string()),
        pid,
        watch.executable,
    );
    map.insert(pid, watch);
}

async fn handle_process_end(
    config: &RwLock<config::Config>,
    map: &mut ProcessWatchMap,
    event: win::ProcessEndResult,
) {
    let event = match event {
        Ok(event) => event,
        Err(error) => {
            warn!("Could not process end event: {:?}", error);
            return;
        }
    };
    let Some(watch) = map.remove(&event.target_instance.process_id) else {
        return;
    };

    let duration_seconds = watch.start.elapsed().as_secs();
    info!(
        "Process {} ({}) ran for {} seconds",
        watch.name.as_ref().unwrap_or(&String::from("?")),
        &watch.executable,
        duration_seconds
    );

    let config = config.read().unwrap();
    let minimum_duration = config.minimum_duration;
    if duration_seconds < minimum_duration.into() {
        info!(
            "Skipping submission: doesn't meet minimum duration of {} seconds",
            minimum_duration
        );
        return;
    }

    let submission = shared::Submission {
        duration: duration_seconds,
        executable: watch.executable,
        name: watch.name,
    };
    submit(&config, submission).await;
}

async fn submit(config: &config::Config, submission: shared::Submission) {
    // TODO: Check/make the URL when the configuration is parsed.
    let Ok(url) = Url::parse(&config.url).and_then(|u| u.join("/submit")) else {
        error!("Could not parse URL {}", &config.url);
        return;
    };

    let client = reqwest::Client::new();
    let mut request = client.post(url).json(&submission);
    if let Some(secret) = &config.secret {
        request = request.header("X-Secret-Key", secret);
    }
    match request.send().await {
        Ok(response) => {
            let status_code = response.status();
            match status_code {
                StatusCode::CREATED => info!("Event submitted to the server"),
                StatusCode::INTERNAL_SERVER_ERROR => {
                    info!("Error submitting event: unknown server error.")
                }
                StatusCode::UNAUTHORIZED => error!(
                    "Error submitting event: unauthorized. Double check secret key settings."
                ),
                _ => warn!("Unknown response from the server: {}", status_code),
            }
        }
        Err(error) => {
            error!("Could not submit event to server: {}", error);
            return;
        }
    };
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    SimpleLogger::new()
        .with_level(LevelFilter::Info)
        .env()
        .init()
        .unwrap();

    let Ok(config_path) = config::Config::get_path() else {
        error!("Could not determine configuration path");
        return Ok(());
    };
    let config = match config::Config::load(&config_path) {
        Ok(config) => Arc::new(RwLock::new(config)),
        Err(_) => {
            error!("Could not load configuration");
            return Ok(());
        }
    };
    info!("Loaded configuration");

    // Reload the configuration if the config file is changed.
    let w_config = config.clone();
    let mut watcher =
        notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
            Ok(event) => {
                if let Ok(new_config) = config::Config::load(event.paths[0].as_path()) {
                    let mut config_write = w_config.write().unwrap();
                    *config_write = new_config;
                }
            }
            Err(e) => warn!("Error monitoring configuration file: {}", e),
        })?;
    match watcher.watch(&config_path.as_path(), notify::RecursiveMode::NonRecursive) {
        Ok(()) => debug!("Monitoring {} for changes", &config_path.display()),
        Err(_) => warn!(
            "Can't monitor config file {} for changes",
            &config_path.display()
        ),
    };

    let (mut stream_start, mut stream_end) = match win::create_streams() {
        Ok((start, end)) => (start, end),
        _ => return Ok(()),
    };

    let mut process_watch = ProcessWatchMap::new();
    info!("Listening to events");
    loop {
        tokio::select! {
            Some(event) = stream_start.next() => handle_process_start(&config, &mut process_watch, event).await,
            Some(event) = stream_end.next() => handle_process_end(&config, &mut process_watch, event).await,
            else => break,
        }
    }
    Ok(())
}
