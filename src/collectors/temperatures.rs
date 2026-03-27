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
            s.push_field(label, format!("{temp:.1} C  (max: {max:.1} C)"));
        }

        // sysinfo cannot read Windows hardware sensors directly.
        // Fall back to Open Hardware Monitor's WMI bridge if it is running.
        #[cfg(target_os = "windows")]
        if !found {
            found = collect_ohm_windows(&mut s)?;
        }

        if !found {
            s.push_field(
                "Note",
                "No temperature sensors found. On Windows, Open Hardware Monitor must be running as Administrator.",
            );
        }

        return Ok(vec![s]);
    }
}

/// Query Open Hardware Monitor's WMI namespace for temperature sensors.
/// OHM must be running as Administrator for this namespace to be populated.
#[cfg(target_os = "windows")]
fn collect_ohm_windows(s: &mut Section) -> Result<bool, ProfilerError> {
    use serde::Deserialize;
    use wmi::WMIConnection;

    #[derive(Deserialize, Debug)]
    struct Sensor {
        #[serde(rename = "Name")]
        name: String,
        #[serde(rename = "SensorType")]
        sensor_type: String,
        #[serde(rename = "Value")]
        value: f32,
        #[serde(rename = "Max")]
        max: f32,
        #[serde(rename = "Parent")]
        parent: String,
    }

    let com = wmi::COMLibrary::new()
        .map_err(|e| ProfilerError::Wmi(e.to_string()))?;

    // If OHM is not running this namespace won't exist — treat as "not found".
    let conn = match WMIConnection::with_namespace_path("ROOT\\OpenHardwareMonitor", com) {
        Ok(c) => c,
        Err(_) => return Ok(false),
    };

    let sensors: Vec<Sensor> = match conn.query() {
        Ok(s) => s,
        Err(_) => return Ok(false),
    };

    let mut found = false;
    for sensor in sensors {
        if sensor.sensor_type != "Temperature" {
            continue;
        }
        found = true;
        let label = format!("{} ({})", sensor.name, sensor.parent);
        s.push_field(label, format!("{:.1} C  (max: {:.1} C)", sensor.value, sensor.max));
    }

    return Ok(found);
}
