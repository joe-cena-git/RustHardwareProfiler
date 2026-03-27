/// Unified error type for all collectors.
#[derive(Debug, thiserror::Error)]
pub enum ProfilerError {
    #[error("WMI query failed: {0}")]
    Wmi(String),

    #[error("NVML error: {0}")]
    Nvml(String),

    #[error("sysinfo error: {0}")]
    SysInfo(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("platform not supported: {0}")]
    Unsupported(String),

    #[error("{0}")]
    Other(String),
}
