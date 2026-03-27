use crate::collectors::Collector;
use crate::error::ProfilerError;
use crate::report::Section;
use sysinfo::System;

pub struct OsCollector;

impl Collector for OsCollector {
    fn section_title(&self) -> &'static str {
        return "OPERATING SYSTEM";
    }

    fn collect(&self) -> Result<Vec<Section>, ProfilerError> {
        let mut s: Section = Section::untitled();

        s.push_field("OS",           System::long_os_version().unwrap_or_default());
        s.push_field("Kernel",       System::kernel_version().unwrap_or_default());
        s.push_field("Hostname",     System::host_name().unwrap_or_default());
        s.push_field("Architecture", std::env::consts::ARCH.to_string());

        // Uptime
        let uptime_secs: u64 = System::uptime();
        let uptime_str: String = format!(
            "{}d {}h {}m",
            uptime_secs / 86400,
            (uptime_secs % 86400) / 3600,
            (uptime_secs % 3600) / 60
        );
        s.push_field("Uptime", uptime_str);

        return Ok(vec![s]);
    }
}
