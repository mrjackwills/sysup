use crate::app_env::AppEnv;
use crate::app_error::AppError;
use auto_launch::{AutoLaunch, AutoLaunchBuilder};
use std::env;

use super::Service;

pub struct WindowsService;

impl WindowsService {
    fn get_auto_launch() -> Result<AutoLaunch, AppError> {
        let exe_path = env::current_exe()?;
        Ok(AutoLaunchBuilder::new()
            .set_app_name(env!("CARGO_PKG_NAME"))
            .set_app_path(exe_path.display().to_string().as_str())
            .build()?)
    }

    /// Install service
    fn service_install() -> Result<(), AppError> {
        let auto_launch = Self::get_auto_launch()?;
        auto_launch.enable().ok();
        Ok(())
    }

    /// remove service
    fn service_uninstall() -> Result<(), AppError> {
        let auto_launch = Self::get_auto_launch()?;
        auto_launch.disable().ok();
        Ok(())
    }
}

impl Service for WindowsService {
    fn uninstall(_: &AppEnv) -> Result<(), AppError> {
        Self::service_uninstall()
    }

    fn install(_: &AppEnv) -> Result<(), AppError> {
        Self::service_uninstall()?;
        Self::service_install()
    }
}
