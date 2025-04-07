use crate::app_error::AppError;
use directories::BaseDirs;
use jiff::tz::TimeZone;
use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
};

type EnvHashMap = HashMap<String, String>;

#[derive(Debug, Clone)]
pub struct AppEnv {
    pub location_base: PathBuf,
    pub location_sqlite: PathBuf,
    pub location_lock: PathBuf,
    pub log_level: tracing::Level,
    pub timezone: TimeZone,
    pub token_app: String,
    pub token_user: String,
    pub machine_name: String,
}

impl AppEnv {
    #[cfg(target_os = "windows")]
    fn get_base() -> PathBuf {
        BaseDirs::new()
            .map_or_else(
                || PathBuf::from("."),
                |f| f.config_local_dir().to_path_buf(),
            )
            .join(env!("CARGO_PKG_NAME"))
    }

    /// FIX me - issue with sudo when running in linux - see https://github.com/dirs-dev/dirs-rs/issues/29
    #[cfg(target_os = "linux")]
    fn get_base() -> PathBuf {
        use crate::service_install::LinuxService;

        LinuxService::get_sudo_user_name().map_or_else(
            || {
                BaseDirs::new()
                    .map_or_else(|| PathBuf::from("."), |f| f.config_dir().to_path_buf())
                    .join(env!("CARGO_PKG_NAME"))
            },
            |name| {
                PathBuf::from("/home")
                    .join(name)
                    .join(".config")
                    .join(env!("CARGO_PKG_NAME"))
            },
        )
    }

    // Get the config location, will create directory if doesn't already exist
    fn get_location() -> Result<PathBuf, AppError> {
        let base = Self::get_base();
        if !std::fs::exists(&base).unwrap_or_default() {
            std::fs::create_dir(&base)?;
        }
        Ok(base)
    }

    fn location_database(location: &Path) -> PathBuf {
        location.join("database").with_extension("db")
    }

    fn location_lock(location: &Path) -> PathBuf {
        location.join("lock")
    }

    /// Parse "true" or "false" to bool, else false
    fn parse_boolean(key: &str, map: &EnvHashMap) -> bool {
        map.get(key).is_some_and(|value| value == "true")
    }

    /// Parse debug and/or trace into tracing level
    fn parse_log(map: &EnvHashMap) -> tracing::Level {
        if Self::parse_boolean("LOG_TRACE", map) {
            tracing::Level::TRACE
        } else if Self::parse_boolean("LOG_DEBUG", map) {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        }
    }

    fn parse_string(key: &str, map: &EnvHashMap) -> Result<String, AppError> {
        map.get(key)
            .map_or(Err(AppError::MissingEnv(key.into())), |value| {
                Ok(value.into())
            })
    }

    /// Check that a given timezone is valid, else return UTC
    fn parse_timezone(map: &EnvHashMap) -> TimeZone {
        map.get("TIMEZONE").map_or(TimeZone::UTC, |s| {
            jiff::tz::TimeZone::get(s).unwrap_or(TimeZone::UTC)
        })
    }

    /// Load, and parse .env file, return AppEnv
    fn generate() -> Result<Self, AppError> {
        let env_map = env::vars()
            .map(|i| (i.0, i.1))
            .collect::<HashMap<String, String>>();

        let base = Self::get_location()?;
        Ok(Self {
            location_lock: Self::location_lock(&base),
            location_sqlite: Self::location_database(&base),
            location_base: base,
            log_level: Self::parse_log(&env_map),
            timezone: Self::parse_timezone(&env_map),
            token_app: Self::parse_string("TOKEN_APP", &env_map)?,
            token_user: Self::parse_string("TOKEN_USER", &env_map)?,
            machine_name: Self::parse_string("MACHINE_NAME", &env_map)?,
        })
    }

    pub fn get() -> Self {
        let current_exe_dir =
            env::current_exe().map_or(None, |p| p.ancestors().nth(1).map(|i| i.join(".env")));

        let env_path = dotenvy::dotenv().unwrap_or_else(|_| {
            current_exe_dir.map_or_else(
                || {
                    println!("\n\x1b[31munable to load env file\x1b[0m\n");
                    std::process::exit(1);
                },
                |current_exe_dir| current_exe_dir,
            )
        });

        dotenvy::from_path(env_path).ok();
        match Self::generate() {
            Ok(s) => s,
            Err(e) => {
                println!("\n\x1b[31m{e}\x1b[0m\n");
                std::process::exit(1);
            }
        }
    }

    /// Delete the lock file
    pub fn rm_lock_file(&self) {
        std::fs::remove_file(&self.location_lock).ok();
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use crate::S;

    use super::*;

    #[test]
    fn env_missing_env() {
        let mut map = HashMap::new();
        map.insert(S!("not_fish"), S!("not_fish"));
        // ACTION
        let result = AppEnv::parse_string("fish", &map);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "missing env: 'fish'");
    }

    #[test]
    fn env_parse_string_valid() {
        let mut map = HashMap::new();
        map.insert(S!("LOCATION_SQLITE"), S!("/alarms.db"));

        // ACTION
        let result = AppEnv::parse_string("LOCATION_SQLITE", &map).unwrap();

        assert_eq!(result, "/alarms.db");
    }

    #[test]
    fn env_parse_boolean_ok() {
        let mut map = HashMap::new();
        map.insert(S!("valid_true"), S!("true"));
        map.insert(S!("valid_false"), S!("false"));
        map.insert(S!("invalid_but_false"), S!("as"));

        // ACTION
        let result01 = AppEnv::parse_boolean("valid_true", &map);
        let result02 = AppEnv::parse_boolean("valid_false", &map);
        let result03 = AppEnv::parse_boolean("invalid_but_false", &map);
        let result04 = AppEnv::parse_boolean("missing", &map);

        assert!(result01);
        assert!(!result02);
        assert!(!result03);
        assert!(!result04);
    }

    #[test]
    fn env_parse_timezone_ok() {
        // FIXTURES
        let mut map = HashMap::new();
        map.insert(S!("TIMEZONE"), S!("America/New_York"));

        // ACTION
        let result = AppEnv::parse_timezone(&map);

        // CHECK
        assert_eq!(result.iana_name(), Some("America/New_York"));

        let mut map = HashMap::new();
        map.insert(S!("TIMEZONE"), S!("Europe/Berlin"));

        // ACTION
        let result = AppEnv::parse_timezone(&map);

        // CHECK
        assert_eq!(result.iana_name(), Some("Europe/Berlin"));

        // FIXTURES
        let map = HashMap::new();

        // ACTION
        let result = AppEnv::parse_timezone(&map);

        // CHECK
        assert_eq!(result.iana_name(), Some("UTC"));
    }

    #[test]
    fn env_parse_timezone_err() {
        // FIXTURES
        let mut map = HashMap::new();
        map.insert(S!("TIMEZONE"), S!("america/New__York"));

        // ACTION
        let result = AppEnv::parse_timezone(&map);
        // CHECK
        assert_eq!(result.iana_name(), Some("UTC"));

        // No timezone present
        // FIXTURES
        let map = HashMap::new();
        let result = AppEnv::parse_timezone(&map);

        // CHECK
        assert_eq!(result.iana_name(), Some("UTC"));
    }

    #[test]
    fn env_parse_log_valid() {
        let map = HashMap::from([(S!("RANDOM_STRING"), S!("123"))]);

        // ACTION
        let result = AppEnv::parse_log(&map);

        assert_eq!(result, tracing::Level::INFO);

        let map = HashMap::from([(S!("LOG_DEBUG"), S!("false"))]);

        // ACTION
        let result = AppEnv::parse_log(&map);

        assert_eq!(result, tracing::Level::INFO);

        let map = HashMap::from([(S!("LOG_TRACE"), S!("false"))]);

        // ACTION
        let result = AppEnv::parse_log(&map);

        assert_eq!(result, tracing::Level::INFO);

        let map = HashMap::from([
            (S!("LOG_DEBUG"), S!("false")),
            (S!("LOG_TRACE"), S!("false")),
        ]);

        // ACTION
        let result = AppEnv::parse_log(&map);

        assert_eq!(result, tracing::Level::INFO);

        let map = HashMap::from([
            (S!("LOG_DEBUG"), S!("true")),
            (S!("LOG_TRACE"), S!("false")),
        ]);

        // ACTION
        let result = AppEnv::parse_log(&map);

        assert_eq!(result, tracing::Level::DEBUG);

        let map = HashMap::from([(S!("LOG_DEBUG"), S!("true")), (S!("LOG_TRACE"), S!("true"))]);

        // ACTION
        let result = AppEnv::parse_log(&map);

        assert_eq!(result, tracing::Level::TRACE);

        let map = HashMap::from([
            (S!("LOG_DEBUG"), S!("false")),
            (S!("LOG_TRACE"), S!("true")),
        ]);

        // ACTION
        let result = AppEnv::parse_log(&map);

        assert_eq!(result, tracing::Level::TRACE);
    }

    // #[test]
    // fn env_panic_appenv() {
    //     // ACTION
    //     let result = AppEnv::generate();

    //     assert!(result.is_err());
    // }

    #[test]
    fn env_return_appenv() {
        dotenvy::dotenv().ok();

        // ACTION
        let result = AppEnv::generate();

        assert!(result.is_ok());
    }
}
