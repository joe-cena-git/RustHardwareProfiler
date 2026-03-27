use crate::collectors::Collector;
use crate::error::ProfilerError;
use crate::report::{fmt_bytes, Section};

pub struct GpuCollector;

impl Collector for GpuCollector {
    fn section_title(&self) -> &'static str {
        return "GRAPHICS (GPU)";
    }

    fn collect(&self) -> Result<Vec<Section>, ProfilerError> {
        let mut sections: Vec<Section> = Vec::new();

        // NVIDIA via NVML — most detailed path, works cross-platform
        #[cfg(feature = "nvidia")]
        sections.extend(collect_nvidia()?);

        // Platform fallbacks for non-NVIDIA or when NVML is unavailable
        #[cfg(target_os = "windows")]
        if sections.is_empty() {
            sections.extend(collect_wmi_gpus()?);
        }

        #[cfg(target_os = "linux")]
        if sections.is_empty() {
            sections.extend(collect_sysfs_gpus()?);
        }

        if sections.is_empty() {
            let mut s: Section = Section::untitled();
            s.push_field("Note", "No GPU info available. NVML / platform APIs returned no results.".to_string());
            sections.push(s);
        }

        return Ok(sections);
    }
}

/// NVIDIA GPUs via nvml-wrapper. Requires NVIDIA drivers installed.
#[cfg(feature = "nvidia")]
fn collect_nvidia() -> Result<Vec<Section>, ProfilerError> {
    use nvml_wrapper::Nvml;

    let nvml: Nvml = Nvml::init()
        .map_err(|e| ProfilerError::Nvml(e.to_string()))?;

    let device_count: u32 = nvml
        .device_count()
        .map_err(|e| ProfilerError::Nvml(e.to_string()))?;

    let mut sections: Vec<Section> = Vec::new();

    for index in 0..device_count {
        let dev = nvml
            .device_by_index(index)
            .map_err(|e| ProfilerError::Nvml(e.to_string()))?;

        let name: String = dev.name()
            .unwrap_or_else(|_| format!("GPU {index}"));

        let mut s: Section = Section::new(name.clone());

        // VRAM
        if let Ok(mem) = dev.memory_info() {
            s.push_subfield("VRAM Total", fmt_bytes(mem.total));
            s.push_subfield("VRAM Used",  fmt_bytes(mem.used));
            s.push_subfield("VRAM Free",  fmt_bytes(mem.free));
        }

        // Clocks
        if let Ok(clk) = dev.clock_info(nvml_wrapper::enum_wrappers::device::Clock::Graphics) {
            s.push_subfield("GPU Clock", format!("{clk} MHz"));
        }
        if let Ok(clk) = dev.clock_info(nvml_wrapper::enum_wrappers::device::Clock::Memory) {
            s.push_subfield("Memory Clock", format!("{clk} MHz"));
        }

        // Temp & fan
        if let Ok(temp) = dev.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu) {
            s.push_subfield("Temperature", format!("{temp} C"));
        }
        if let Ok(fan) = dev.fan_speed(0) {
            s.push_subfield("Fan Speed", format!("{fan}%"));
        }

        // Power
        if let Ok(power_mw) = dev.power_usage() {
            s.push_subfield("Power Draw",  format!("{:.1} W", power_mw as f64 / 1000.0));
        }
        if let Ok(limit_mw) = dev.enforced_power_limit() {
            s.push_subfield("Power Limit", format!("{:.1} W", limit_mw as f64 / 1000.0));
        }

        // Utilization
        if let Ok(util) = dev.utilization_rates() {
            s.push_subfield("GPU Utilization",    format!("{}%", util.gpu));
            s.push_subfield("Memory Utilization", format!("{}%", util.memory));
        }

        // PCIe
        if let Ok(gen) = dev.current_pcie_link_gen() {
            s.push_subfield("PCIe Gen", gen.to_string());
        }
        if let Ok(width) = dev.current_pcie_link_width() {
            s.push_subfield("PCIe Width", format!("x{width}"));
        }

        // Driver / CUDA version from NVML top-level
        if let Ok(driver) = nvml.sys_driver_version() {
            s.push_subfield("Driver Version", driver);
        }
        if let Ok(cuda) = nvml.sys_cuda_driver_version() {
            s.push_subfield("CUDA Version", format!("{}.{}", cuda / 1000, (cuda % 1000) / 10));
        }

        sections.push(s);
    }

    return Ok(sections);
}

/// Windows fallback: WMI Win32_VideoController.
#[cfg(target_os = "windows")]
fn collect_wmi_gpus() -> Result<Vec<Section>, ProfilerError> {
    use wmi::WMIConnection;
    use serde::Deserialize;

    #[allow(non_camel_case_types)]
    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    struct Win32_VideoController {
        name: Option<String>,
        driver_version: Option<String>,
        adapter_ram: Option<u32>,  // WMI DWORD — saturates at 4 GB
        video_mode_description: Option<String>,
        current_refresh_rate: Option<u32>,
        status: Option<String>,
    }

    let wmi: WMIConnection = crate::collectors::wmi_connect()?;

    let controllers: Vec<Win32_VideoController> = wmi
        .query()
        .map_err(|e| ProfilerError::Wmi(e.to_string()))?;

    let mut sections: Vec<Section> = Vec::new();

    for vc in controllers {
        let name: String = vc.name.clone().unwrap_or_else(|| "Unknown".to_string());
        let mut s: Section = Section::new(name);

        if let Some(driver) = vc.driver_version { s.push_subfield("Driver Version", driver); }
        if let Some(vram) = vc.adapter_ram {
            // Skip sentinel values: 0 = unknown, u32::MAX = overflowed (card > 4 GB)
            if vram > 0 && vram < u32::MAX {
                s.push_subfield("VRAM", fmt_bytes(vram as u64));
            }
        }
        if let Some(mode) = vc.video_mode_description { s.push_subfield("Video Mode", mode); }
        if let Some(hz) = vc.current_refresh_rate { s.push_subfield("Refresh Rate", format!("{hz} Hz")); }
        if let Some(status) = vc.status { s.push_subfield("Status", status); }

        sections.push(s);
    }

    return Ok(sections);
}

/// Linux fallback: scan /sys/class/drm for GPU entries.
#[cfg(target_os = "linux")]
fn collect_sysfs_gpus() -> Result<Vec<Section>, ProfilerError> {
    use std::fs;

    let drm_path: std::path::PathBuf = std::path::PathBuf::from("/sys/class/drm");
    if !drm_path.exists() {
        return Ok(Vec::new());
    }

    let mut sections: Vec<Section> = Vec::new();

    for entry in fs::read_dir(&drm_path).map_err(ProfilerError::Io)? {
        let entry = entry.map_err(ProfilerError::Io)?;
        let name: String = entry.file_name().to_string_lossy().to_string();

        // Only top-level card entries, not connector entries
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }

        let device_path: std::path::PathBuf = entry.path().join("device");
        let vendor: String = fs::read_to_string(device_path.join("vendor"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let device: String = fs::read_to_string(device_path.join("device"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        let mut s: Section = Section::new(name);
        s.push_subfield("Vendor ID", vendor);
        s.push_subfield("Device ID", device);

        sections.push(s);
    }

    return Ok(sections);
}
