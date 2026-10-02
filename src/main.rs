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
fn tokio_signal() {
    tokio::spawn(async {
        tokio::signal::ctrl_c().await.ok();
        exit("ctrl+c", &Code::Invalid);
    });
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let cli: CliArgs = CliArgs::new();
    let app_env = AppEnv::get();

    tokio_signal();

    let lock_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .truncate(false)
        .create(true)
        .open(&app_env.location_lock)?;
    let mut lock = RwLock::new(lock_file);

    // Guard is held until main returns, blocking any concurrent instance
    let Ok(_lock_guard) = lock.try_write() else {
        return Ok(());
    };

    setup_tracing(&app_env)?;
    let db = init_db(&app_env).await?;

    match service_install::check(&cli, &app_env, &db).await {
        Ok(Some(status)) => {
            PushRequest::from(status)
                .make_request(&app_env, &db)
                .await?;
        }
        Ok(None) => {
            if let Some(skip_request) = ModelSkipRequest::get(&db).await
                && !skip_request.skip
            {
                PushRequest::Online.make_request(&app_env, &db).await?;
            }
        }
        Err(e) => {
            tracing::error!("service (un)install failed: {e}");
        }
    }

    Ok(())
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use jiff::tz::TimeZone;
    use sqlx::SqlitePool;
    use uuid::Uuid;

    use super::*;
    use std::path::PathBuf;

    pub fn gen_app_env(name: Uuid) -> AppEnv {
        AppEnv {
            timezone: TimeZone::UTC,
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
        let mut app_env = gen_app_env(uuid);
        app_env.timezone = TimeZone::get("Europe/London").unwrap();
        let db = init_db(&app_env).await.unwrap();
        (app_env, db, uuid)
    }

    #[tokio::test]
    /// A second handle to the same lock file must fail to acquire the lock
    async fn lock_file_blocks_second_instance() {
        let mut app_env = gen_app_env(Uuid::new_v4());
        app_env.location_lock = app_env
            .location_lock
            .with_file_name(format!("lock_{}", Uuid::new_v4().simple()));

        let open_lock_file = || -> std::io::Result<std::fs::File> {
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .truncate(false)
                .create(true)
                .open(&app_env.location_lock)
        };

        let mut first = RwLock::new(open_lock_file().unwrap());
        // Guard must be kept bound, else the lock is released immediately
        let _write_guard = first.try_write().unwrap();

        let mut second = RwLock::new(open_lock_file().unwrap());
        assert!(second.try_write().is_err());

        std::fs::remove_file(&app_env.location_lock).ok();
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
        let sql_sham = sql_name.with_extension("db-shm");
        let sql_wal = sql_name.with_extension("db-wal");
        tokio::fs::remove_file(sql_wal).await.ok();
        tokio::fs::remove_file(sql_sham).await.ok();
        tokio::fs::remove_file(sql_name).await.ok();
    }
}
