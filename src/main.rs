mod collectors;
mod error;
mod report;

use anyhow::Result;
use clap::Parser;
use collectors::{
    cpu::CpuCollector,
    gpu::GpuCollector,
    installed::InstalledCollector,
    memory::MemoryCollector,
    motherboard::MotherboardCollector,
    network::NetworkCollector,
    os::OsCollector,
    runtimes::RuntimesCollector,
    storage::StorageCollector,
    temperatures::TemperatureCollector,
    Collector,
};
use report::Report;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "hwprofile", version, about = "Cross-platform hardware profiler")]
struct Args {
    /// Output file path. Defaults to hwprofile_HOSTNAME_TIMESTAMP.txt in the current directory.
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,

    /// Emit JSON instead of plain text.
    #[arg(short, long)]
    json: bool,

    /// Redact serial numbers, MAC addresses, and UUIDs.
    #[arg(short, long)]
    redact: bool,

    /// Suppress stdout — write to file only.
    #[arg(short, long)]
    quiet: bool,
}

fn main() -> Result<()> {
    let args: Args = Args::parse();

    // Build ordered list of collectors. Each runs independently —
    // a failure in one section does not abort the rest.
    let collectors: Vec<Box<dyn Collector>> = vec![
        Box::new(OsCollector),
        Box::new(MotherboardCollector),
        Box::new(CpuCollector),
        Box::new(MemoryCollector),
        Box::new(GpuCollector),
        Box::new(StorageCollector),
        Box::new(NetworkCollector),
        Box::new(TemperatureCollector),
        Box::new(RuntimesCollector),
        Box::new(InstalledCollector),
    ];

    let mut report: Report = Report::new(args.redact);

    for collector in &collectors {
        match collector.collect() {
            Ok(sections) => report.add_sections(collector.section_title(), sections),
            Err(e) => report.add_error(collector.section_title(), &e.to_string()),
        }
    }

    report.add_summary();

    // Resolve output path
    let output_path: PathBuf = match args.output {
        Some(p) => p,
        None => {
            let hostname: String = hostname();
            let timestamp: String = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
            PathBuf::from(format!("hwprofile_{hostname}_{timestamp}.txt"))
        }
    };

    // Render and write
    if args.json {
        let json: String = report.to_json()?;
        if !args.quiet {
            println!("{json}");
        }
        std::fs::write(&output_path, &json)?;
    } else {
        let text: String = report.to_text();
        if !args.quiet {
            print!("{text}");
        }
        std::fs::write(&output_path, &text)?;
    }

    eprintln!("Report saved to: {}", output_path.display());

    return Ok(());
}

fn hostname() -> String {
    // Prefer the environment variable, fall back to sysinfo
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| {
            sysinfo::System::host_name().unwrap_or_else(|| "unknown".to_string())
        })
}
