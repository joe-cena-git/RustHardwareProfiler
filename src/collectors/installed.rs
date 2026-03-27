use crate::collectors::Collector;
use crate::error::ProfilerError;
use crate::report::Section;
use std::process::Command;

pub struct InstalledCollector;

impl Collector for InstalledCollector {
    fn section_title(&self) -> &'static str {
        return "INSTALLED PROGRAMS";
    }

    fn collect(&self) -> Result<Vec<Section>, ProfilerError> {
        #[cfg(target_os = "windows")]
        return collect_windows();

        #[cfg(not(target_os = "windows"))]
        return Ok(vec![Section::untitled().field(
            "Note",
            "Installed program listing not implemented on this platform.",
        )]);
    }
}

#[cfg(target_os = "windows")]
fn collect_windows() -> Result<Vec<Section>, ProfilerError> {
    // Query all three uninstall hives: 64-bit, 32-bit (WOW), and per-user.
    let ps = r#"
$keys = @(
    'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*'
)
Get-ItemProperty $keys -ErrorAction SilentlyContinue |
    Where-Object { $_.DisplayName } |
    Select-Object DisplayName, DisplayVersion, Publisher |
    Sort-Object DisplayName |
    ConvertTo-Json -Compress
"#;

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", ps])
        .output()
        .map_err(|e| ProfilerError::Other(format!("PowerShell unavailable: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ProfilerError::Other(format!("PowerShell error: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim())
        .map_err(|e| ProfilerError::Other(format!("JSON parse error: {e}")))?;

    // PowerShell returns an object (not array) when there is exactly one result.
    let entries: Vec<&serde_json::Value> = match &json {
        serde_json::Value::Array(arr) => arr.iter().collect(),
        obj => vec![obj],
    };

    let mut s = Section::untitled();

    for entry in entries {
        let name = entry["DisplayName"].as_str().unwrap_or("").trim().to_string();
        if name.is_empty() {
            continue;
        }
        let version = entry["DisplayVersion"].as_str().unwrap_or("").trim().to_string();
        let value = if version.is_empty() { "installed".to_string() } else { version };
        s.push_field(name, value);
    }

    return Ok(vec![s]);
}
