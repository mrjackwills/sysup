use sqlx::SqlitePool;

use crate::{app_env::AppEnv, app_error::AppError, db::ModelSkipRequest, parse_cli::CliArgs};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::LinuxService;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows::WindowsService;

trait Service {
    fn uninstall(app_env: &AppEnv) -> Result<(), AppError>;
    fn install(app_env: &AppEnv) -> Result<(), AppError>;
}

pub enum Status {
    Install,
    Uninstall,
}

impl Status {
    pub const fn get<'a>(&self) -> &'a str {
        match self {
            Self::Install => "service installed",
            Self::Uninstall => "service uninstalled",
        }
    }
}

/// check the cli args, and perform (un)install if necessary
pub async fn check(
    cli: &CliArgs,
    app_envs: &AppEnv,
    db: &SqlitePool,
) -> Result<Option<Status>, AppError> {
    if cli.install {
        tracing::info!("Attempting to install service");
        #[cfg(target_os = "linux")]
        LinuxService::install(app_envs)?;
        #[cfg(target_os = "windows")]
        WindowsService::install(app_envs)?;
        ModelSkipRequest::update(db, false).await?;
        Ok(Some(Status::Install))
    } else if cli.uninstall {
        tracing::info!("Attempting to uninstall service");
        #[cfg(target_os = "linux")]
        LinuxService::uninstall(app_envs)?;
        #[cfg(target_os = "windows")]
        WindowsService::uninstall(app_envs)?;
        ModelSkipRequest::update(db, true).await?;
        Ok(Some(Status::Uninstall))
    } else {
        Ok(None)
    }
}
