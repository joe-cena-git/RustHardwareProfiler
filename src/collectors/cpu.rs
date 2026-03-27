use crate::collectors::Collector;
use crate::error::ProfilerError;
use crate::report::{fmt_mhz, Section};
use sysinfo::{CpuRefreshKind, RefreshKind, System};

pub struct CpuCollector;

impl Collector for CpuCollector {
    fn section_title(&self) -> &'static str {
        return "PROCESSOR (CPU)";
    }

    fn collect(&self) -> Result<Vec<Section>, ProfilerError> {
        // Two samples are required for an accurate usage delta.
        let mut sys: System = System::new_with_specifics(
            RefreshKind::new().with_cpu(CpuRefreshKind::everything()),
        );
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        sys.refresh_cpu_all();

        let cpus: &[sysinfo::Cpu] = sys.cpus();
        if cpus.is_empty() {
            return Err(ProfilerError::SysInfo("no CPUs found".to_string()));
        }

        // sysinfo exposes logical processors — group by physical package via brand string.
        // For most consumer machines this is one physical CPU.
        let brand: String = cpus[0].brand().to_string();
        let logical_count: usize = cpus.len();

        // Physical core count is not directly exposed by sysinfo on all platforms.
        // Use the physical_core_count() helper where available.
        let physical_cores: String = sys
            .physical_core_count()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let freq_mhz: u64 = cpus[0].frequency();
        let load: f32 = cpus.iter().map(|c| c.cpu_usage()).sum::<f32>() / logical_count as f32;

        let mut s: Section = Section::untitled();
        s.push_field("Name",               brand);
        s.push_field("Physical Cores",     physical_cores);
        s.push_field("Logical Processors", logical_count.to_string());
        s.push_field("Base Clock",         fmt_mhz(freq_mhz));
        s.push_field("Load (current)",     format!("{load:.1}%"));

        return Ok(vec![s]);
    }
}
