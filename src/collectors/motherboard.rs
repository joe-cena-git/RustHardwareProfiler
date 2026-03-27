use crate::collectors::Collector;
use crate::error::ProfilerError;
use crate::report::Section;

pub struct MotherboardCollector;

impl Collector for MotherboardCollector {
    fn section_title(&self) -> &'static str {
        return "MOTHERBOARD & BIOS";
    }

    fn collect(&self) -> Result<Vec<Section>, ProfilerError> {
        #[cfg(target_os = "windows")]
        return collect_windows();

        #[cfg(target_os = "linux")]
        return collect_linux();

        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        return Ok(vec![Section::untitled().field(
            "Note",
            "Motherboard detail not available on this platform.",
        )]);
    }
}

#[cfg(target_os = "windows")]
fn collect_windows() -> Result<Vec<Section>, ProfilerError> {
    use wmi::WMIConnection;
    use serde::Deserialize;

    #[allow(non_camel_case_types)]
    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    struct Win32_BaseBoard {
        manufacturer: Option<String>,
        product: Option<String>,
        version: Option<String>,
        serial_number: Option<String>,
    }

    #[allow(non_camel_case_types)]
    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    struct Win32_BIOS {
        manufacturer: Option<String>,
        #[serde(rename = "SMBIOSBIOSVersion")]
        smbios_bios_version: Option<String>,
        release_date: Option<String>,
        serial_number: Option<String>,
    }

    let wmi: WMIConnection = crate::collectors::wmi_connect()?;

    let mut sections: Vec<Section> = Vec::new();

    // Motherboard
    let boards: Vec<Win32_BaseBoard> = wmi.query()
        .map_err(|e| ProfilerError::Wmi(e.to_string()))?;

    if let Some(board) = boards.into_iter().next() {
        let mut s: Section = Section::new("Motherboard");
        if let Some(v) = board.manufacturer  { s.push_subfield("Manufacturer", v); }
        if let Some(v) = board.product       { s.push_subfield("Product",      v); }
        if let Some(v) = board.version       { s.push_subfield("Version",      v); }
        if let Some(v) = board.serial_number { s.push_subfield("Serial",       v); }
        sections.push(s);
    }

    // BIOS
    let bios_records: Vec<Win32_BIOS> = wmi.query()
        .map_err(|e| ProfilerError::Wmi(e.to_string()))?;

    if let Some(bios) = bios_records.into_iter().next() {
        let mut s: Section = Section::new("BIOS");
        if let Some(v) = bios.manufacturer       { s.push_subfield("Manufacturer", v); }
        if let Some(v) = bios.smbios_bios_version { s.push_subfield("Version",     v); }
        if let Some(v) = bios.release_date       { s.push_subfield("Release Date", parse_wmi_date(&v)); }
        if let Some(v) = bios.serial_number      { s.push_subfield("Serial",       v); }
        sections.push(s);
    }

    return Ok(sections);
}

/// Parse WMI CIM_DATETIME "YYYYMMDDHHmmss.uuuuuu+UUU" → "YYYY-MM-DD".
#[cfg(target_os = "windows")]
fn parse_wmi_date(s: &str) -> String {
    if s.len() >= 8 && s.chars().take(8).all(|c| c.is_ascii_digit()) {
        return format!("{}-{}-{}", &s[0..4], &s[4..6], &s[6..8]);
    }
    return s.to_string();
}

#[cfg(target_os = "linux")]
fn collect_linux() -> Result<Vec<Section>, ProfilerError> {
    use std::fs;

    let dmi: std::path::PathBuf = std::path::PathBuf::from("/sys/class/dmi/id");
    if !dmi.exists() {
        return Err(ProfilerError::Unsupported(
            "DMI sysfs not available — may need root".to_string(),
        ));
    }

    let read = |file: &str| -> String {
        fs::read_to_string(dmi.join(file))
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    };

    let mut board: Section = Section::new("Motherboard");
    board.push_subfield("Manufacturer", read("board_vendor"));
    board.push_subfield("Product",      read("board_name"));
    board.push_subfield("Version",      read("board_version"));
    board.push_subfield("Serial",       read("board_serial"));

    let mut bios: Section = Section::new("BIOS");
    bios.push_subfield("Vendor",       read("bios_vendor"));
    bios.push_subfield("Version",      read("bios_version"));
    bios.push_subfield("Release Date", read("bios_date"));

    return Ok(vec![board, bios]);
}
