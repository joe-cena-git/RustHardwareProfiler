use crate::collectors::Collector;
use crate::error::ProfilerError;
use crate::report::Section;
use sysinfo::Networks;

pub struct NetworkCollector;

impl Collector for NetworkCollector {
    fn section_title(&self) -> &'static str {
        return "NETWORK";
    }

    fn collect(&self) -> Result<Vec<Section>, ProfilerError> {
        let networks: Networks = Networks::new_with_refreshed_list();
        let mut sections: Vec<Section> = Vec::new();

        for (name, data) in networks.iter() {
            let mut s: Section = Section::new(name.clone());
            s.push_subfield("MAC Address",   data.mac_address().to_string());
            s.push_subfield("Received",      fmt_throughput(data.total_received()));
            s.push_subfield("Transmitted",   fmt_throughput(data.total_transmitted()));
            sections.push(s);
        }

        return Ok(sections);
    }
}

fn fmt_throughput(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        return format!("{:.2} GB", bytes as f64 / 1_000_000_000.0);
    }
    if bytes >= 1_000_000 {
        return format!("{:.2} MB", bytes as f64 / 1_000_000.0);
    }
    return format!("{:.2} KB", bytes as f64 / 1_000.0);
}
