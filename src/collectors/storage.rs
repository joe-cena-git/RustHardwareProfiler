use crate::collectors::Collector;
use crate::error::ProfilerError;
use crate::report::{fmt_bytes, fmt_pct, Section};
use sysinfo::Disks;

pub struct StorageCollector;

impl Collector for StorageCollector {
    fn section_title(&self) -> &'static str {
        return "STORAGE";
    }

    fn collect(&self) -> Result<Vec<Section>, ProfilerError> {
        let disks: Disks = Disks::new_with_refreshed_list();
        let mut sections: Vec<Section> = Vec::new();

        // Logical volumes / mount points from sysinfo
        let mut logical: Section = Section::new("Logical Volumes");
        for disk in disks.list() {
            let name: String = disk.name().to_string_lossy().to_string();
            let mount: String = disk.mount_point().to_string_lossy().to_string();
            let total: u64    = disk.total_space();
            let free: u64     = disk.available_space();
            let used: u64     = total.saturating_sub(free);
            let fs: String    = disk.file_system().to_string_lossy().to_string();
            let kind: String  = format!("{:?}", disk.kind());

            logical.push_field(
                format!("{} ({})", name, mount),
                format!(
                    "{} total — {} used ({}) — {} free — {} {}",
                    fmt_bytes(total),
                    fmt_bytes(used),
                    fmt_pct(used, total),
                    fmt_bytes(free),
                    fs,
                    kind,
                ),
            );
        }
        sections.push(logical);

        // Physical disk detail — platform-specific
        #[cfg(target_os = "windows")]
        sections.extend(collect_physical_windows()?);

        #[cfg(target_os = "linux")]
        sections.extend(collect_physical_linux()?);

        return Ok(sections);
    }
}

#[cfg(target_os = "windows")]
fn collect_physical_windows() -> Result<Vec<Section>, ProfilerError> {
    use wmi::WMIConnection;
    use serde::Deserialize;

    #[allow(non_camel_case_types)]
    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    struct Win32_DiskDrive {
        model: Option<String>,
        size: Option<u64>,
        serial_number: Option<String>,
        firmware_revision: Option<String>,
        interface_type: Option<String>,
        status: Option<String>,
        media_type: Option<String>,
    }

    let wmi: WMIConnection = crate::collectors::wmi_connect()?;

    let drives: Vec<Win32_DiskDrive> = wmi
        .query()
        .map_err(|e| ProfilerError::Wmi(e.to_string()))?;

    let mut sections: Vec<Section> = Vec::new();

    for (i, drive) in drives.iter().enumerate() {
        let model: String = drive.model.clone().unwrap_or_else(|| format!("Disk {i}"));
        let mut s: Section = Section::new(format!("Physical Disk {i} - {model}"));

        if let Some(size) = drive.size { s.push_subfield("Size", fmt_bytes(size)); }
        if let Some(ref iface) = drive.interface_type { s.push_subfield("Interface", iface.clone()); }
        if let Some(ref media) = drive.media_type { s.push_subfield("Media Type", media.clone()); }
        if let Some(ref fw) = drive.firmware_revision { s.push_subfield("Firmware", fw.clone()); }
        if let Some(ref serial) = drive.serial_number { s.push_subfield("Serial Number", serial.trim().to_string()); }
        if let Some(ref status) = drive.status { s.push_subfield("Status", status.clone()); }

        sections.push(s);
    }

    return Ok(sections);
}

#[cfg(target_os = "linux")]
fn collect_physical_linux() -> Result<Vec<Section>, ProfilerError> {
    use std::process::Command;

    let output: std::process::Output = Command::new("lsblk")
        .args(["-d", "-o", "NAME,SIZE,TYPE,ROTA,MODEL,SERIAL,TRAN,STATE", "--json"])
        .output()
        .map_err(|e| ProfilerError::Other(format!("lsblk unavailable: {e}")))?;

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| ProfilerError::Other(format!("lsblk parse error: {e}")))?;

    let mut sections: Vec<Section> = Vec::new();

    if let Some(devices) = json["blockdevices"].as_array() {
        for dev in devices {
            let name: &str  = dev["name"].as_str().unwrap_or("unknown");
            let model: &str = dev["model"].as_str().unwrap_or("unknown");
            let mut s: Section = Section::new(format!("/dev/{name} - {model}"));

            s.push_subfield("Size",          dev["size"].as_str().unwrap_or_default());
            s.push_subfield("Transport",     dev["tran"].as_str().unwrap_or_default());
            s.push_subfield("Rotational",    if dev["rota"].as_str() == Some("1") { "HDD" } else { "SSD/NVMe" });
            s.push_subfield("Serial",        dev["serial"].as_str().unwrap_or_default());
            s.push_subfield("State",         dev["state"].as_str().unwrap_or_default());

            sections.push(s);
        }
    }

    return Ok(sections);
}
