#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use app_env::AppEnv;
use app_error::AppError;
use db::{ModelSkipRequest, init_db};
use fd_lock::RwLock;
use parse_cli::CliArgs;
use request::PushRequest;
use tracing_subscriber::{fmt, layer::SubscriberExt};

mod app_env;
mod app_error;
mod db;
mod parse_cli;
mod request;
mod service_install;

const LOGS_NAME: &str = "log";

/// Simple macro to create a new String, or convert from a &str to  a String - basically just gets rid of String::from() / .to_owned() etc
#[macro_export]
macro_rules! S {
    () => {
        String::new()
    };
    ($s:expr) => {
        String::from($s)
    };
}

/// Simple macro to call `.clone()` on whatever is passed in
#[macro_export]
macro_rules! C {
    ($i:expr) => {
        $i.clone()
    };
}

pub enum Code {
    Valid,
    Invalid,
}

/// Global process exit, with message and code
pub fn exit(message: &str, code: &Code) {
    match code {
        Code::Valid => {
            tracing::info!(message);
            std::process::exit(0);
        }
        Code::Invalid => {
            tracing::error!(message);
            std::process::exit(1);
        }
    }
}

// Tracing to a file and stdout
fn setup_tracing(app_env: &AppEnv) -> Result<(), AppError> {
    let logfile = tracing_appender::rolling::never(&app_env.location_base, LOGS_NAME);

    let log_fmt = fmt::Layer::default()
        .json()
        .flatten_event(true)
        .with_writer(logfile);

    match tracing::subscriber::set_global_default(
        fmt::Subscriber::builder()
            .with_file(true)
            .with_line_number(true)
            .with_max_level(app_env.log_level)
            .finish()
            .with(log_fmt),
    ) {
        Ok(()) => Ok(()),
        Err(e) => {
            println!("{e:?}");
            Err(AppError::Tracing)
        }
    }
}

/// Spawn a thread to watch for exit signals, so can show cursor correctly
fn tokio_signal(app_envs: &AppEnv) {
    let app_envs = C!(app_envs);
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        app_envs.rm_lock_file();
        exit("ctrl+c", &Code::Invalid);
    });
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let cli: CliArgs = CliArgs::new();
    let app_envs = AppEnv::get();

    tokio_signal(&app_envs);
    let lock_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .truncate(false)
        .create(true)
        .open(&app_envs.location_lock)?;
    let mut lock_file = RwLock::new(lock_file);
    let single_instance = lock_file.try_write();

    if single_instance.is_ok() {
        setup_tracing(&app_envs)?;
        let db = init_db(&app_envs).await?;

        if let Ok(str) = service_install::check(&cli, &app_envs, &db).await {
            if let Some(status) = str {
                PushRequest::from(status)
                    .make_request(&app_envs, &db)
                    .await?;
            } else if let Some(skip_request) = ModelSkipRequest::get(&db).await {
                if !skip_request.skip {
                    PushRequest::Online.make_request(&app_envs, &db).await?;
                }
            }
        }
        app_envs.rm_lock_file();
    }

    Ok(())
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use sqlx::SqlitePool;
    use uuid::Uuid;

    use crate::app_env::EnvTimeZone;

    use super::*;
    use std::path::PathBuf;

    pub fn gen_app_envs(name: Uuid) -> AppEnv {
        AppEnv {
            timezone: EnvTimeZone::new("Europe/London"),
            log_level: tracing::Level::INFO,
            token_app: S!("test_token_app"),
            token_user: S!("test_token_user"),
            machine_name: S!("test_machine"),

            #[cfg(target_os = "linux")]
            location_sqlite: PathBuf::from(format!("/dev/shm/{name}.db")),
            #[cfg(target_os = "linux")]
            location_lock: PathBuf::from("/dev/shm/lock"),
            #[cfg(target_os = "linux")]
            location_base: PathBuf::from("/dev/shm"),

            #[cfg(target_os = "windows")]
            location_lock: PathBuf::from("./windows_tests/lock"),
            #[cfg(target_os = "windows")]
            location_base: PathBuf::from("./windows_tests"),
            #[cfg(target_os = "windows")]
            location_sqlite: PathBuf::from(format!("./windows_tests/{name}.db")),
        }
    }

    pub async fn setup_test() -> (AppEnv, SqlitePool, Uuid) {
        let uuid = Uuid::new_v4();
        let app_envs = gen_app_envs(uuid);
        let db = init_db(&app_envs).await.unwrap();
        (app_envs, db, uuid)
    }

    /// Close database connection, and delete all test files
    pub async fn test_cleanup(uuid: Uuid, db: Option<SqlitePool>) {
        if let Some(db) = db {
            db.close().await;
        }
        #[cfg(target_os = "linux")]
        let sql_name = PathBuf::from(format!("/dev/shm/{uuid}.db"));
        #[cfg(target_os = "windows")]
        let sql_name = std::env::current_dir()
            .unwrap()
            .join("windows_tests")
            .join(format!("{uuid}.db"));
        let sql_sham = sql_name.join("-shm");
        let sql_wal = sql_name.join("-wal");
        tokio::fs::remove_file(sql_wal).await.ok();
        tokio::fs::remove_file(sql_sham).await.ok();
        tokio::fs::remove_file(sql_name).await.ok();
    }
}
