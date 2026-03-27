use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const SEPARATOR_WIDE: &str = "========================================================================";
const SEPARATOR_MID: &str = "--------------------------------------------------------";
const FIELD_WIDTH: usize = 32;

/// A single labeled field within a section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub label: String,
    pub value: String,
    /// Additional indentation levels beyond the base section indent.
    pub indent: u8,
}

/// A named group of fields within a section (e.g. one RAM stick, one disk).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    /// Optional sub-header. None = fields render directly under the parent section.
    pub title: Option<String>,
    pub fields: Vec<Field>,
}

impl Section {
    pub fn new(title: impl Into<String>) -> Self {
        return Self { title: Some(title.into()), fields: Vec::new() };
    }

    pub fn untitled() -> Self {
        return Self { title: None, fields: Vec::new() };
    }

    pub fn field(mut self, label: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.push(Field { label: label.into(), value: value.into(), indent: 0 });
        return self;
    }

    pub fn push_field(&mut self, label: impl Into<String>, value: impl Into<String>) {
        self.fields.push(Field { label: label.into(), value: value.into(), indent: 0 });
    }

    pub fn push_subfield(&mut self, label: impl Into<String>, value: impl Into<String>) {
        self.fields.push(Field { label: label.into(), value: value.into(), indent: 1 });
    }
}

/// Top-level report. Holds ordered sections keyed by collector title,
/// plus a summary and any per-section errors.
#[derive(Debug, Default)]
pub struct Report {
    redact: bool,
    sections: Vec<(String, Vec<Section>)>,
}

impl Report {
    pub fn new(redact: bool) -> Self {
        return Self { redact, sections: Vec::new() };
    }

    pub fn add_sections(&mut self, title: &str, sections: Vec<Section>) {
        self.sections.push((title.to_string(), sections));
    }

    pub fn add_error(&mut self, title: &str, message: &str) {
        let section: Section = Section::untitled()
            .field("Error", message);
        self.sections.push((title.to_string(), vec![section]));
    }

    pub fn add_summary(&mut self) {
        // Summary is populated by main after all collectors run.
        // Collectors can push to it via a separate summary() method if needed.
    }

    // -------------------------------------------------------------------------
    // Plain-text rendering
    // -------------------------------------------------------------------------

    pub fn to_text(&self) -> String {
        let mut out: String = String::with_capacity(8192);
        let hostname: String = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "unknown".to_string());
        let now: String = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        // Report header
        out.push_str(&format!("{SEPARATOR_WIDE}\n"));
        out.push_str(&format!("  HARDWARE PROFILE REPORT\n"));
        out.push_str(&format!("  Generated : {now}\n"));
        out.push_str(&format!("  Machine   : {hostname}\n"));
        out.push_str(&format!("{SEPARATOR_WIDE}\n"));

        for (title, sections) in &self.sections {
            out.push_str(&format!("\n{SEPARATOR_WIDE}\n"));
            out.push_str(&format!("  {title}\n"));
            out.push_str(&format!("{SEPARATOR_WIDE}\n"));

            for section in sections {
                if let Some(ref sub_title) = section.title {
                    out.push_str(&format!("\n  {sub_title}\n"));
                    out.push_str(&format!("  {SEPARATOR_MID}\n"));
                }

                for field in &section.fields {
                    let base_indent: usize = 4 + (field.indent as usize * 2);
                    let pad: usize = FIELD_WIDTH.saturating_sub(field.label.len() + field.indent as usize * 2);
                    let label_padded: String = format!(
                        "{}{}{} ",
                        " ".repeat(base_indent),
                        field.label,
                        " ".repeat(pad)
                    );
                    let value: &str = if self.redact {
                        &self.maybe_redact(&field.value)
                    } else {
                        &field.value
                    };
                    out.push_str(&format!("{label_padded}: {value}\n"));
                }
            }
        }

        out.push_str(&format!("\n{SEPARATOR_WIDE}\n"));
        out.push_str(&format!("  END OF REPORT - {now}\n"));
        out.push_str(&format!("{SEPARATOR_WIDE}\n\n"));

        return out;
    }

    // -------------------------------------------------------------------------
    // JSON rendering
    // -------------------------------------------------------------------------

    pub fn to_json(&self) -> Result<String> {
        // Build a map of section title -> array of { title, fields }
        let mut map: BTreeMap<String, serde_json::Value> = BTreeMap::new();

        for (title, sections) in &self.sections {
            let serialized: Vec<serde_json::Value> = sections
                .iter()
                .map(|s: &Section| {
                    let fields: Vec<serde_json::Value> = s
                        .fields
                        .iter()
                        .map(|f: &Field| {
                            let value: String = if self.redact {
                                self.maybe_redact(&f.value)
                            } else {
                                f.value.clone()
                            };
                            serde_json::json!({ "label": f.label, "value": value })
                        })
                        .collect();

                    serde_json::json!({
                        "title": s.title,
                        "fields": fields,
                    })
                })
                .collect();

            map.insert(title.clone(), serde_json::Value::Array(serialized));
        }

        return Ok(serde_json::to_string_pretty(&map)?);
    }

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    fn maybe_redact(&self, value: &str) -> String {
        // Redact values that look like serial numbers, MACs, or UUIDs.
        // Heuristic: hex strings of 8+ chars, MAC patterns, UUID patterns.
        // Genuine model names / version strings are left intact.
        let mac_re: bool = value.contains(':') && value.len() == 17;
        let uuid_like: bool = value.len() == 36 && value.chars().filter(|c| *c == '-').count() == 4;
        let serial_like: bool = value.len() >= 8
            && value.chars().all(|c| c.is_ascii_alphanumeric())
            && value.chars().any(|c| c.is_ascii_digit())
            && value.chars().any(|c| c.is_ascii_uppercase());

        if mac_re || uuid_like || serial_like {
            return "[REDACTED]".to_string();
        }

        return value.to_string();
    }
}

// -------------------------------------------------------------------------
// Formatting helpers for use in collectors
// -------------------------------------------------------------------------

pub fn fmt_bytes(bytes: u64) -> String {
    const TB: u64 = 1_000_000_000_000;
    const GB: u64 = 1_000_000_000;
    const MB: u64 = 1_000_000;
    const KB: u64 = 1_000;

    if bytes >= TB { return format!("{:.2} TB", bytes as f64 / TB as f64); }
    if bytes >= GB { return format!("{:.2} GB", bytes as f64 / GB as f64); }
    if bytes >= MB { return format!("{:.2} MB", bytes as f64 / MB as f64); }
    if bytes >= KB { return format!("{:.2} KB", bytes as f64 / KB as f64); }
    return format!("{bytes} B");
}

pub fn fmt_mhz(mhz: u64) -> String {
    if mhz >= 1000 {
        return format!("{:.2} GHz ({mhz} MHz)", mhz as f64 / 1000.0);
    }
    return format!("{mhz} MHz");
}

pub fn fmt_pct(used: u64, total: u64) -> String {
    if total == 0 { return "0%".to_string(); }
    return format!("{:.1}%", (used as f64 / total as f64) * 100.0);
}
