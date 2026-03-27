/// Unified error type for all collectors.
#[derive(Debug, thiserror::Error)]
pub enum ProfilerError {
    #[error("WMI query failed: {0}")]
    Wmi(String),

    #[error("GPU error: {0}")]
    Gpu(String),

    #[error("sysinfo error: {0}")]
    SysInfo(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[cfg(target_os = "linux")]
    #[error("platform not supported: {0}")]
    Unsupported(String),

    #[error("{0}")]
    Other(String),
}
