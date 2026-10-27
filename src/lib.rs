mod app;
pub mod cli;
mod completion;
pub mod cronrc;
pub mod crontab;
pub mod hook;
pub mod runner;
pub mod store;
pub use app::execute;
pub use tokio_util::sync::CancellationToken;
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
    #[error(
        "authorization refused for {dir}: {status}; run cron-person dir allow after reviewing .cronrc"
    )]
    Authorization { dir: String, status: store::Status },
    #[error("command exited with {0}")]
    Child(std::process::ExitStatus),
    #[error("crontab {operation} failed: {message}")]
    Provider {
        operation: &'static str,
        message: String,
    },
    #[error("crontab changed during synchronization; inspect it before retrying")]
    Conflict,
    #[error("operation cancelled")]
    Cancelled,
    #[error("state committed, but directory synchronization failed: {0}")]
    Committed(std::io::Error),
}
pub fn exit_status(error: &Error) -> i32 {
    match error {
        Error::Cancelled => 130,
        Error::Committed(_) => 6,
        _ => 1,
    }
}
