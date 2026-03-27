use crate::collectors::Collector;
use crate::error::ProfilerError;
use crate::report::Section;
use sysinfo::Components;

pub struct TemperatureCollector;

impl Collector for TemperatureCollector {
    fn section_title(&self) -> &'static str {
        return "TEMPERATURES";
    }

    fn collect(&self) -> Result<Vec<Section>, ProfilerError> {
        let components: Components = Components::new_with_refreshed_list();
        let mut s: Section = Section::untitled();
        let mut found: bool = false;

        for component in components.iter() {
            found = true;
            let label: &str = component.label();
            let temp: f32   = component.temperature();
            let max: f32    = component.max();

            s.push_field(
                label,
                format!("{temp:.1} C  (max: {max:.1} C)"),
            );
        }

        if !found {
            s.push_field(
                "Note",
                "No temperature sensors found. On Windows, install and run Open Hardware Monitor as Administrator.",
            );
        }

        return Ok(vec![s]);
    }
}
