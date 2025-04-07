use crate::app_env::AppEnv;
use crate::app_error::AppError;
use crate::{Code, exit};
use std::{env, fs, io::Write, path::Path, process::Command};
use tracing::debug;

use super::Service;

const SYSTEMCTL: &str = "systemctl";
const APP_NAME: &str = env!("CARGO_PKG_NAME");
const CHOWN: &str = "chown";

pub struct LinuxService;

impl LinuxService {
    // Get user name when running as sudo, to check if is sudo
    pub fn get_sudo_user_name() -> Option<String> {
        std::env::var("SUDO_USER").map_or(None, |user_name| {
            if user_name == "root" || user_name.is_empty() {
                None
            } else {
                Some(user_name)
            }
        })
    }

    /// Check if we're running as sudo
    fn check_sudo() {
        match sudo::check() {
            sudo::RunningAs::Root => (),
            _ => exit("not running as sudo", &Code::Invalid),
        }
    }

    /// Get service name for systemd service
    fn get_service_name() -> String {
        format!("{APP_NAME}.service")
    }

    /// Get filename for systemd service file
    fn get_dot_service() -> String {
        let service = Self::get_service_name();
        format!("/etc/systemd/system/{service}")
    }

    /// Create a systemd service file, with correct details
    fn create_service_file(user_name: &str) -> Result<String, AppError> {
        let current_dir = env::current_dir()?.display().to_string();
        Ok(format!(
            "[Unit]
Description={APP_NAME}
After=network-online.target
Wants=network-online.target
StartLimitIntervalSec=0

[Service]
ExecStart={current_dir}/{APP_NAME}
WorkingDirectory={current_dir}
SyslogIdentifier={APP_NAME}
User={user_name}
Group={user_name}
Restart=no

[Install]
WantedBy=multi-user.target"
        ))
    }

    /// Change the ownership of the config and it's content
    fn chown_config(user_name: &str, app_envs: &AppEnv) -> Result<(), AppError> {
        Command::new(CHOWN)
            .args([
                "-R",
                &format!("{user_name}:{user_name}"),
                app_envs.location_base.display().to_string().as_str(),
            ])
            .output()?;
        Ok(())
    }

    /// If is sudo, and able to get a user name (which isn't root), install leafcast as a service
    #[expect(clippy::cognitive_complexity)]
    fn systemd_install(app_envs: &AppEnv) -> Result<(), AppError> {
        if let Some(user_name) = Self::get_sudo_user_name() {
            Self::chown_config(&user_name, app_envs)?;

            debug!("Create service file");
            let mut file = fs::File::create(Self::get_dot_service())?;

            debug!("Write unit text to file");
            file.write_all(Self::create_service_file(&user_name)?.as_bytes())?;

            debug!("Reload systemctl daemon");
            Command::new(SYSTEMCTL).arg("daemon-reload").output()?;

            let service_name = Self::get_service_name();
            debug!("Enable service");
            Command::new(SYSTEMCTL)
                .args(["enable", &service_name])
                .output()?;
        }
        Ok(())
    }

    /// check if unit file in systemd, and delete if true
    #[expect(clippy::cognitive_complexity)]
    fn systemd_uninstall(app_envs: &AppEnv) -> Result<(), AppError> {
        if let Some(user_name) = Self::get_sudo_user_name() {
            Self::chown_config(&user_name, app_envs)?;
            let service = Self::get_service_name();

            let path = Self::get_dot_service();

            if Path::new(&path).exists() {
                debug!("Stopping service");
                Command::new(SYSTEMCTL).args(["stop", &service]).output()?;

                debug!("Disabling service");
                Command::new(SYSTEMCTL)
                    .args(["disable", &service])
                    .output()?;

                debug!("Removing service file");
                std::fs::remove_file(path)?;

                debug!("Reload daemon-service");
                Command::new(SYSTEMCTL).arg("daemon-reload").output()?;
            }
        }
        Ok(())
    }
}

impl Service for LinuxService {
    fn uninstall(app_envs: &AppEnv) -> Result<(), AppError> {
        Self::check_sudo();
        Self::systemd_uninstall(app_envs)
    }

    fn install(app_envs: &AppEnv) -> Result<(), AppError> {
        Self::uninstall(app_envs)?;
        Self::systemd_install(app_envs)
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    /// systemd service name correctly created
    fn test_systemd_create_get_service_name() {
        let result = LinuxService::get_service_name();

        assert_eq!(result, "sysup.service");
    }

    #[test]
    /// systemd unit file name/location correctly created
    fn test_systemd_create_get_dot_service() {
        let result = LinuxService::get_dot_service();

        assert_eq!(result, "/etc/systemd/system/sysup.service");
    }

    #[test]
    /// Systemd unti file is created correctly
    fn test_systemd_create_service_file() {
        let result = LinuxService::create_service_file("test_user");
        assert!(result.is_ok());

        let expected = "[Unit]\nDescription=sysup\nAfter=network-online.target\nWants=network-online.target\nStartLimitIntervalSec=0\n\n[Service]\nExecStart=/workspaces/sysup/sysup\nWorkingDirectory=/workspaces/sysup\nSyslogIdentifier=sysup\nUser=test_user\nGroup=test_user\nRestart=no\n\n[Install]\nWantedBy=multi-user.target";
        assert_eq!(result.unwrap(), expected);
    }
}
