pub mod cpu;
pub mod gpu;
pub mod installed;
pub mod memory;
pub mod motherboard;
pub mod network;
pub mod os;
pub mod runtimes;
pub mod storage;
pub mod temperatures;

use crate::error::ProfilerError;
use crate::report::Section;

/// Open a WMI connection. `WMIConnection` takes ownership of `COMLibrary`
/// and keeps COM initialised for its lifetime.
#[cfg(target_os = "windows")]
pub(crate) fn wmi_connect() -> Result<wmi::WMIConnection, ProfilerError> {
    let com = wmi::COMLibrary::new()
        .map_err(|e| ProfilerError::Wmi(e.to_string()))?;
    wmi::WMIConnection::new(com)
        .map_err(|e| ProfilerError::Wmi(e.to_string()))
}

/// Every hardware category implements this trait.
/// Failures are isolated per-collector — one broken section does not abort others.
pub trait Collector: Send + Sync {
    /// Section header shown in the report.
    fn section_title(&self) -> &'static str;

    /// Collect data and return formatted sections.
    /// Returns an empty Vec on platforms where this data is unavailable.
    fn collect(&self) -> Result<Vec<Section>, ProfilerError>;
}
