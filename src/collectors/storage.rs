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

        #[cfg(target_os = "macos")]
        sections.extend(collect_physical_macos()?);

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

/// macOS: enumerate physical disks via diskutil list -plist and diskutil info.
#[cfg(target_os = "macos")]
fn collect_physical_macos() -> Result<Vec<Section>, ProfilerError> {
    use std::process::Command;

    let list_output = Command::new("diskutil")
        .args(["list", "-plist", "physical"])
        .output()
        .map_err(|e| ProfilerError::Other(format!("diskutil unavailable: {e}")))?;

    if !list_output.status.success() {
        return Ok(Vec::new());
    }

    // Parse WholeDisks array from the plist.
    let plist_text = String::from_utf8_lossy(&list_output.stdout);
    let mut disk_names: Vec<String> = Vec::new();

    let mut in_whole_disks = false;
    for line in plist_text.lines() {
        let line = line.trim();
        if line.contains("WholeDisks") {
            in_whole_disks = true;
            continue;
        }
        if in_whole_disks {
            if line == "</array>" { break; }
            if line.starts_with("<string>") && line.ends_with("</string>") {
                disk_names.push(line[8..line.len() - 9].to_string());
            }
        }
    }

    let mut sections: Vec<Section> = Vec::new();

    for disk in disk_names {
        let info_output = Command::new("diskutil")
            .args(["info", "-plist", &format!("/dev/{disk}")])
            .output();

        let Ok(info) = info_output else { continue };
        if !info.status.success() { continue; }

        let info_text = String::from_utf8_lossy(&info.stdout);

        let read_val = |key: &str| -> String {
            let needle = format!("<key>{key}</key>");
            if let Some(pos) = info_text.find(&needle) {
                let rest = &info_text[pos + needle.len()..];
                if let Some(s) = rest.find("<string>") {
                    if let Some(e) = rest.find("</string>") {
                        return rest[s + 8..e].trim().to_string();
                    }
                }
                if let Some(s) = rest.find("<integer>") {
                    if let Some(e) = rest.find("</integer>") {
                        return rest[s + 9..e].trim().to_string();
                    }
                }
            }
            return String::new();
        };

        let media_name = read_val("MediaName");
        let title = if media_name.is_empty() {
            format!("/dev/{disk}")
        } else {
            format!("/dev/{disk} - {media_name}")
        };

        let mut s = Section::new(title);

        let size_bytes: u64 = read_val("TotalSize").parse().unwrap_or(0);
        if size_bytes > 0 {
            s.push_subfield("Size", crate::report::fmt_bytes(size_bytes));
        }
        for (label, key) in &[
            ("Protocol",     "BusProtocol"),
            ("Solid State",  "SolidState"),
            ("Smart Status", "SmartStatus"),
        ] {
            let v = read_val(key);
            if !v.is_empty() { s.push_subfield(*label, v); }
        }

        sections.push(s);
    }

    return Ok(sections);
}
