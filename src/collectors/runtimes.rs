use crate::collectors::Collector;
use crate::error::ProfilerError;
use crate::report::Section;
use std::process::Command;

pub struct RuntimesCollector;

impl Collector for RuntimesCollector {
    fn section_title(&self) -> &'static str {
        return "RUNTIMES & TOOLS";
    }

    fn collect(&self) -> Result<Vec<Section>, ProfilerError> {
        let tools: &[(&str, &[&str])] = &[
            ("rustc",   &["--version"]),
            ("cargo",   &["--version"]),
            ("dotnet",  &["--version"]),
            ("node",    &["--version"]),
            ("python",  &["--version"]),
            ("git",     &["--version"]),
            ("docker",  &["--version"]),
            ("ollama",  &["--version"]),
            ("nvcc",    &["--version"]),
            ("kubectl", &["version", "--client"]),
        ];

        let mut s: Section = Section::untitled();

        for (cmd, args) in tools {
            let version: String = probe_version(cmd, args);
            s.push_field(*cmd, version);
        }

        return Ok(vec![s]);
    }
}

/// Run a command and return its first non-empty output line, or "not found".
fn probe_version(cmd: &str, args: &[&str]) -> String {
    let result: std::io::Result<std::process::Output> = Command::new(cmd)
        .args(args)
        .output();

    return match result {
        Ok(output) => {
            let stdout: String = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr: String = String::from_utf8_lossy(&output.stderr).trim().to_string();

            // Some tools (nvcc) write version to stderr
            let text: &str = if !stdout.is_empty() { &stdout } else { &stderr };

            text.lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("unknown")
                .trim()
                .to_string()
        }
        Err(_) => "not found".to_string(),
    };
}
