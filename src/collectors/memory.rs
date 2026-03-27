use crate::collectors::Collector;
use crate::error::ProfilerError;
use crate::report::{fmt_bytes, fmt_pct, Section};
use sysinfo::{MemoryRefreshKind, RefreshKind, System};

pub struct MemoryCollector;

impl Collector for MemoryCollector {
    fn section_title(&self) -> &'static str {
        return "MEMORY (RAM)";
    }

    fn collect(&self) -> Result<Vec<Section>, ProfilerError> {
        let sys: System = System::new_with_specifics(
            RefreshKind::new().with_memory(MemoryRefreshKind::everything()),
        );

        let total: u64 = sys.total_memory();
        let used: u64  = sys.used_memory();
        let free: u64  = sys.available_memory();

        let mut summary: Section = Section::untitled();
        summary.push_field("Total",        fmt_bytes(total));
        summary.push_field("Used",         format!("{} ({})", fmt_bytes(used), fmt_pct(used, total)));
        summary.push_field("Available",    fmt_bytes(free));

        let mut sections: Vec<Section> = vec![summary];

        // Per-DIMM detail is platform-specific
        #[cfg(target_os = "windows")]
        sections.extend(collect_dimm_detail_windows()?);

        #[cfg(target_os = "linux")]
        sections.extend(collect_dimm_detail_linux()?);

        return Ok(sections);
    }
}

/// Windows: query per-DIMM detail via WMI Win32_PhysicalMemory.
#[cfg(target_os = "windows")]
fn collect_dimm_detail_windows() -> Result<Vec<Section>, ProfilerError> {
    use wmi::WMIConnection;
    use serde::Deserialize;

    #[allow(non_camel_case_types)]
    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    struct Win32_PhysicalMemory {
        capacity: Option<u64>,
        speed: Option<u32>,
        configured_clock_speed: Option<u32>,
        manufacturer: Option<String>,
        part_number: Option<String>,
        serial_number: Option<String>,
        bank_label: Option<String>,
        device_locator: Option<String>,
        #[serde(rename = "SMBIOSMemoryType")]
        smbios_memory_type: Option<u32>,
        form_factor: Option<u32>,
        data_width: Option<u32>,
    }

    let wmi: WMIConnection = crate::collectors::wmi_connect()?;

    let sticks: Vec<Win32_PhysicalMemory> = wmi
        .query()
        .map_err(|e| ProfilerError::Wmi(e.to_string()))?;

    let mut sections: Vec<Section> = Vec::new();

    for (i, stick) in sticks.iter().enumerate() {
        let bank: String = stick.bank_label.clone().unwrap_or_default();
        let slot: String = stick.device_locator.clone().unwrap_or_default();
        let title: String = format!("Stick {} - {} / {}", i + 1, bank, slot);

        let mut s: Section = Section::new(title);

        if let Some(cap) = stick.capacity {
            s.push_subfield("Capacity", fmt_bytes(cap));
        }
        s.push_subfield("Type",             smbios_memory_type(stick.smbios_memory_type.unwrap_or(0)));
        s.push_subfield("Speed",            format!("{} MHz", stick.speed.unwrap_or(0)));
        s.push_subfield("Configured Speed", format!("{} MHz", stick.configured_clock_speed.unwrap_or(0)));
        s.push_subfield("Manufacturer",     stick.manufacturer.clone().unwrap_or_default().trim().to_string());
        s.push_subfield("Part Number",      stick.part_number.clone().unwrap_or_default().trim().to_string());
        s.push_subfield("Serial Number",    stick.serial_number.clone().unwrap_or_default());
        s.push_subfield("Form Factor",      form_factor(stick.form_factor.unwrap_or(0)));
        s.push_subfield("Data Width",       format!("{}-bit", stick.data_width.unwrap_or(0)));

        sections.push(s);
    }

    return Ok(sections);
}

/// Linux: parse per-DIMM detail from dmidecode output.
/// Requires root / sudo for dmidecode to return actual values.
#[cfg(target_os = "linux")]
fn collect_dimm_detail_linux() -> Result<Vec<Section>, ProfilerError> {
    use std::process::Command;

    let output: std::process::Output = Command::new("dmidecode")
        .args(["--type", "17"])
        .output()
        .map_err(|e| ProfilerError::Other(format!("dmidecode unavailable: {e}")))?;

    if !output.status.success() {
        return Err(ProfilerError::Other(
            "dmidecode requires root — run with sudo for per-DIMM detail".to_string(),
        ));
    }

    let text: String = String::from_utf8_lossy(&output.stdout).to_string();
    let mut sections: Vec<Section> = Vec::new();
    let mut current: Option<Section> = None;
    let mut stick_index: u32 = 0;

    for line in text.lines() {
        let line: &str = line.trim();

        if line.starts_with("Memory Device") {
            if let Some(s) = current.take() { sections.push(s); }
            stick_index += 1;
            current = Some(Section::new(format!("Stick {stick_index}")));
            continue;
        }

        if let Some(ref mut s) = current {
            if let Some((key, val)) = line.split_once(':') {
                let key: &str = key.trim();
                let val: &str = val.trim();
                match key {
                    "Size" | "Type" | "Speed" | "Configured Memory Speed" |
                    "Manufacturer" | "Part Number" | "Serial Number" |
                    "Bank Locator" | "Locator" | "Form Factor" | "Data Width" => {
                        s.push_subfield(key, val);
                    }
                    _ => {}
                }
            }
        }
    }

    if let Some(s) = current { sections.push(s); }

    return Ok(sections);
}

#[cfg(target_os = "windows")]
fn smbios_memory_type(code: u32) -> String {
    return match code {
        20 => "DDR".to_string(),
        21 => "DDR2".to_string(),
        24 => "DDR3".to_string(),
        26 => "DDR4".to_string(),
        34 => "DDR5".to_string(),
        _  => format!("Type {code}"),
    };
}

#[cfg(target_os = "windows")]
fn form_factor(code: u32) -> String {
    return match code {
        8  => "DIMM".to_string(),
        12 => "SO-DIMM".to_string(),
        _  => format!("Code {code}"),
    };
}
