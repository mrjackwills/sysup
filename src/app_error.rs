use std::num::TryFromIntError;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[cfg(target_os = "windows")]
    #[error("Autolaunch error: {0}")]
    AutoLaunch(#[from] auto_launch::Error),
    #[error("Url parsing error: {0}")]
    Convert(#[from] TryFromIntError),
    #[error("IO Error")]
    IOError(#[from] std::io::Error),
    #[error("missing env: '{0}'")]
    MissingEnv(String),
    #[error("No network connection")]
    Offline,
    #[error("Reqwest Error")]
    Reqwest(#[from] reqwest::Error),
    #[error("Internal Database Error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("Unable to set up tracing")]
    Tracing,
    #[error("Url parsing error: {0}")]
    Url(#[from] url::ParseError),
}
