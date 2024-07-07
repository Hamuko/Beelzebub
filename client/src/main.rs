use futures::StreamExt;
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use log::{debug, error, info, warn, LevelFilter};
use notify::Watcher;
use simple_logger::SimpleLogger;
use std::sync::{Arc, RwLock};

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

fn handle_process_start(
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

fn handle_process_end(map: &mut ProcessWatchMap, event: win::ProcessEndResult) {
    let event = match event {
        Ok(event) => event,
        Err(error) => {
            warn!("Could not process end event: {:?}", error);
            return;
        }
    };
    if let Some(watch) = map.remove(&event.target_instance.process_id) {
        info!(
            "Process {} ran for {} seconds",
            watch.executable,
            watch.start.elapsed().as_secs()
        );
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    SimpleLogger::new()
        .with_level(LevelFilter::Info)
        .env()
        .init()
        .unwrap();

    let config_path = match config::Config::get_path() {
        Ok(path) => path,
        Err(_) => {
            error!("Could not determine configuration path");
            return Ok(());
        }
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
            Some(event) = stream_start.next() => handle_process_start(&config, &mut process_watch, event),
            Some(event) = stream_end.next() => handle_process_end(&mut process_watch, event),
            else => break,
        }
    }
    Ok(())
}
