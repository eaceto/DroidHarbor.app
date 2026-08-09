use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    #[error("destination escapes the chosen folder: {0}")]
    NotContained(PathBuf),

    #[error("too many files in one transfer: {count} (limit {limit})")]
    TooManyFiles { count: u64, limit: u64 },

    #[error("transfer of {total_bytes} bytes does not fit in available space")]
    InsufficientSpace { total_bytes: u64 },

    #[error("empty transfer")]
    EmptyTransfer,
}
