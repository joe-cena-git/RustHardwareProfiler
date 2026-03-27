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

        #[cfg(target_os = "linux")]
        return collect_linux();

        #[cfg(target_os = "macos")]
        return collect_macos();

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        return Ok(vec![Section::untitled().field(
            "Note",
            "Installed program listing is not supported on this platform.",
        )]);
    }
}

/// Windows: query all three registry uninstall hives via PowerShell.
#[cfg(target_os = "windows")]
fn collect_windows() -> Result<Vec<Section>, ProfilerError> {
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

/// Linux: try dpkg, then rpm, then pacman — use whichever is available.
#[cfg(target_os = "linux")]
fn collect_linux() -> Result<Vec<Section>, ProfilerError> {
    // Debian/Ubuntu — dpkg-query
    if let Ok(output) = Command::new("dpkg-query")
        .args(["-W", "-f=${Package}\t${Version}\n"])
        .output()
    {
        if output.status.success() {
            let mut s = Section::untitled();
            let text = String::from_utf8_lossy(&output.stdout);
            let mut entries: Vec<(&str, &str)> = text
                .lines()
                .filter_map(|l| l.split_once('\t'))
                .collect();
            entries.sort_by_key(|(name, _)| *name);
            for (name, version) in entries {
                if !name.trim().is_empty() {
                    s.push_field(name.trim(), version.trim());
                }
            }
            return Ok(vec![s]);
        }
    }

    // RPM-based (Fedora, RHEL, openSUSE)
    if let Ok(output) = Command::new("rpm")
        .args(["-qa", "--queryformat", "%{NAME}\t%{VERSION}-%{RELEASE}\n"])
        .output()
    {
        if output.status.success() {
            let mut s = Section::untitled();
            let text = String::from_utf8_lossy(&output.stdout);
            let mut entries: Vec<(&str, &str)> = text
                .lines()
                .filter_map(|l| l.split_once('\t'))
                .collect();
            entries.sort_by_key(|(name, _)| *name);
            for (name, version) in entries {
                if !name.trim().is_empty() {
                    s.push_field(name.trim(), version.trim());
                }
            }
            return Ok(vec![s]);
        }
    }

    // Arch Linux — pacman
    if let Ok(output) = Command::new("pacman").args(["-Q"]).output() {
        if output.status.success() {
            let mut s = Section::untitled();
            let text = String::from_utf8_lossy(&output.stdout);
            let mut entries: Vec<(&str, &str)> = text
                .lines()
                .filter_map(|l| l.split_once(' '))
                .collect();
            entries.sort_by_key(|(name, _)| *name);
            for (name, version) in entries {
                if !name.trim().is_empty() {
                    s.push_field(name.trim(), version.trim());
                }
            }
            return Ok(vec![s]);
        }
    }

    let mut s = Section::untitled();
    s.push_field(
        "Note",
        "No supported package manager found (tried dpkg, rpm, pacman).",
    );
    return Ok(vec![s]);
}

/// macOS: query installed applications via system_profiler.
#[cfg(target_os = "macos")]
fn collect_macos() -> Result<Vec<Section>, ProfilerError> {
    let output = Command::new("system_profiler")
        .args(["SPApplicationsDataType", "-json"])
        .output()
        .map_err(|e| ProfilerError::Other(format!("system_profiler unavailable: {e}")))?;

    if !output.status.success() {
        return Err(ProfilerError::Other("system_profiler returned an error".to_string()));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| ProfilerError::Other(format!("JSON parse error: {e}")))?;

    let apps = json["SPApplicationsDataType"]
        .as_array()
        .ok_or_else(|| ProfilerError::Other("Unexpected system_profiler output format".to_string()))?;

    let mut entries: Vec<(String, String)> = apps
        .iter()
        .filter_map(|app| {
            let name = app["_name"].as_str()?.trim().to_string();
            let version = app["version"].as_str().unwrap_or("").trim().to_string();
            Some((name, version))
        })
        .collect();

    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut s = Section::untitled();
    for (name, version) in entries {
        let value = if version.is_empty() { "installed".to_string() } else { version };
        s.push_field(name, value);
    }

    return Ok(vec![s]);
}
