use std::sync::{Arc, RwLock};

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use deadpool_diesel::postgres::{Manager, Pool};
use diesel::{
    pg::data_types::PgInterval,
    result::{DatabaseErrorKind::UniqueViolation, Error::DatabaseError},
    ExpressionMethods, PgConnection, QueryDsl, RunQueryDsl,
};
use log::{debug, error, info, warn, LevelFilter};
use shared;
use simple_logger::SimpleLogger;

mod config;
mod db;
mod schema;
mod util;

type ConfigReference = Arc<RwLock<config::Config>>;

#[derive(Clone)]
struct AppState {
    config: ConfigReference,
    pool: Pool,
}

fn is_authenticated(headers: &HeaderMap, config: &ConfigReference) -> bool {
    let Ok(config) = config.read() else {
        error!("Authentication error: cannot read configuration");
        return false;
    };
    let Some(secret) = &config.secret else {
        debug!("Secret key not set");
        return true;
    };
    let Ok(x_secret_key) = (match headers.get("x-secret-key") {
        Some(value) => value.to_str(),
        None => {
            debug!("Authentication error: X-Secret-Key not provided");
            return false;
        }
    }) else {
        warn!("Authentication error: X-Secret-Key is not text");
        return false;
    };
    return x_secret_key == secret;
}

fn get_process(conn: &mut PgConnection, payload: &shared::Submission) -> Result<i32, ()> {
    use schema::processes::dsl::*;

    let process_name = payload.name.as_ref().map(|s| util::clean_name(s));

    let mut query = processes
        .limit(1)
        .select(id)
        .filter(executable.eq(&payload.executable))
        .into_boxed();
    if process_name.is_some() {
        query = query.filter(name.eq(process_name));
    } else {
        query = query.filter(name.is_null());
    }
    if let Ok(results) = query.limit(1).select(id).load::<i32>(conn) {
        if let Some(result) = results.first() {
            return Ok(*result);
        }
    }

    match diesel::insert_into(processes)
        .values((executable.eq(&payload.executable), name.eq(process_name)))
        .returning(id)
        .get_results::<i32>(conn)
    {
        Ok(result) => return Ok(result[0]),
        Err(DatabaseError(UniqueViolation, _)) => {
            let mut query = processes
                .limit(1)
                .select(id)
                .filter(executable.eq(&payload.executable))
                .into_boxed();
            if process_name.is_some() {
                query = query.filter(name.eq(process_name));
            } else {
                query = query.filter(name.is_null());
            }
            match query.load::<i32>(conn) {
                Ok(results) => return Ok(results[0]),
                Err(error) => {
                    error!("Unknown database error during SELECT: {}", error);
                    return Err(());
                }
            };
        }
        Err(error) => {
            error!("Unknown database error during INSERT: {}", error);
            debug!("Payload: {:?}", payload);
            return Err(());
        }
    }
}

fn database_error() -> (StatusCode, Json<shared::SubmissionResponse>) {
    let response = shared::SubmissionResponse {
        status: shared::SubmissionResponseStatus::DatabaseError,
    };
    return (StatusCode::INTERNAL_SERVER_ERROR, Json(response));
}

async fn submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<shared::Submission>,
) -> (StatusCode, Json<shared::SubmissionResponse>) {
    if !is_authenticated(&headers, &state.config) {
        let response = shared::SubmissionResponse {
            status: shared::SubmissionResponseStatus::Unauthenticated,
        };
        return (StatusCode::UNAUTHORIZED, Json(response));
    }

    let Ok(conn) = state.pool.get().await else {
        error!("Could not get connection from pool");
        return database_error();
    };
    let result = conn
        .interact(move |conn| {
            use diesel::{ExpressionMethods, RunQueryDsl};
            use schema::events::dsl::*;

            let Ok(process_id) = get_process(conn, &payload) else {
                return Err(());
            };
            let interval = PgInterval::from_microseconds(payload.duration as i64 * 1_000_000);
            match diesel::insert_into(events)
                .values((
                    time.eq(diesel::dsl::now),
                    process.eq(process_id),
                    duration.eq(interval),
                ))
                .execute(conn)
            {
                Ok(_) => info!("Process {} saved", payload.display()),
                Err(error) => {
                    error!("Could not save event for {}: {}", payload.display(), error);
                    return Err(());
                }
            }
            Ok(())
        })
        .await;

    if result.is_err() {
        return database_error();
    }

    let response = shared::SubmissionResponse {
        status: shared::SubmissionResponseStatus::Ok,
    };
    (StatusCode::CREATED, Json(response))
}

#[tokio::main]
async fn main() {
    SimpleLogger::new()
        .with_level(LevelFilter::Info)
        .env()
        .init()
        .unwrap();

    let Ok(config_path) = config::Config::get_path() else {
        error!("Could not determine configuration path");
        return;
    };
    let config = match config::Config::load(&config_path) {
        Ok(config) => config,
        Err(_) => {
            error!("Could not load configuration");
            return;
        }
    };

    let manager = Manager::new(&config.db_url, deadpool_diesel::Runtime::Tokio1);
    let pool = Pool::builder(manager).build().unwrap();
    db::run_migrations(&pool).await;

    let config = Arc::new(RwLock::new(config));
    let shared_state = AppState {
        config: config,
        pool: pool,
    };

    let app = Router::new()
        .route("/submit", post(submit))
        .with_state(shared_state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    info!("Launching server");
    axum::serve(listener, app).await.unwrap();
}
