# RustHardwareProfiler

A cross-platform hardware profiler written in Rust. Produces a detailed,
human-readable report of system hardware — CPU, RAM, GPU, storage, network,
motherboard, BIOS, installed runtimes, and every installed program. Output goes
to stdout and a timestamped file.

```text
========================================================================
  HARDWARE PROFILE REPORT
  Generated : 2026-03-15 09:22:41
  Machine   : DESKTOP-EXAMPLE
========================================================================

  OPERATING SYSTEM       Windows 11 Pro (22631) — x86_64 — up 1d 4h 37m

  MOTHERBOARD & BIOS     MSI MAG X670E TOMAHAWK WIFI
                         AMI BIOS 7E12AMS — 2023-09-06

  PROCESSOR (CPU)        AMD Ryzen 9 7950X — 16C / 32T — 4.50 GHz — 8.2% load

  MEMORY (RAM)           64.00 GB DDR5-6000 (2× 32 GB G.Skill) — 22.14 GB used

  GRAPHICS (GPU)         NVIDIA GeForce RTX 4080 — 16 GB VRAM — 38 C — 28.4 W

  STORAGE                Samsung SSD 990 Pro 1TB (C:\) — WD_BLACK SN850X 2TB (D:\)

  NETWORK                Ethernet — [REDACTED] — 84.21 GB rx / 12.47 GB tx
                         Wi-Fi   — [REDACTED]

  RUNTIMES & TOOLS       rustc 1.77.0 · node v20.12.0 · python 3.12.2 · git 2.44.0

  INSTALLED PROGRAMS     19 entries (Docker Desktop, VS Code, Chrome, ...)
```

See [sample-output.txt](sample-output.txt) for full report output.

---

## Install

**From source:**

```bash
git clone https://github.com/your-username/RustHardwareProfiler
cd RustHardwareProfiler
cargo build --release
# Binary at target/release/hwprofile
```

**NVIDIA GPU detail** requires NVIDIA drivers (NVML ships with the driver — no
extra install needed).

**Temperature sensors** on Windows require
[Open Hardware Monitor](https://openhardwaremonitor.org) running as Administrator.

---

## Usage

```text
hwprofile [OPTIONS]

Options:
  -o, --output <FILE>   Write report to FILE instead of default timestamped path
  -j, --json            Output JSON instead of plain text
  -r, --redact          Redact serial numbers, MAC addresses, and UUIDs
  -q, --quiet           Suppress stdout — file output only
  -h, --help            Print help
  -V, --version         Print version
```

```bash
# Basic run — prints to console, saves to hwprofile_HOSTNAME_TIMESTAMP.txt
hwprofile

# Redacted output safe for sharing
hwprofile --redact --output my-build.txt

# JSON for scripting
hwprofile --json

# Silent — file only (good for SSH / cron)
hwprofile --quiet
```

---

## Sections

| Section | Source |
| --- | --- |
| Operating System | sysinfo |
| Motherboard & BIOS | WMI `Win32_BaseBoard` / `Win32_BIOS` (Windows), DMI sysfs (Linux) |
| Processor (CPU) | sysinfo — two-sample delta for accurate load |
| Memory (RAM) | sysinfo (totals) + WMI `Win32_PhysicalMemory` / dmidecode (per-DIMM) |
| Graphics (GPU) | NVML (NVIDIA, primary) → WMI `Win32_VideoController` fallback |
| Storage | sysinfo (volumes) + WMI `Win32_DiskDrive` / lsblk (physical disks) |
| Network | sysinfo |
| Temperatures | sysinfo (Linux hwmon / OHM on Windows) |
| Runtimes & Tools | subprocess version probes |
| Installed Programs | Windows registry uninstall keys (all three hives) |

---

## Platform Support

| Feature | Windows | Linux | macOS |
| --- | --- | --- | --- |
| OS info | ✅ | ✅ | ✅ |
| CPU | ✅ | ✅ | ✅ |
| RAM (total / used) | ✅ | ✅ | ✅ |
| RAM (per-DIMM detail) | ✅ WMI | ✅ dmidecode | ⚠️ limited |
| GPU (NVIDIA) | ✅ NVML | ✅ NVML | ✅ NVML |
| GPU (other) | ✅ WMI | ⚠️ sysfs | — |
| Storage | ✅ | ✅ | ✅ |
| Network | ✅ | ✅ | ✅ |
| Motherboard / BIOS | ✅ WMI | ✅ DMI sysfs | — |
| Temperatures | ✅ OHM | ✅ hwmon | ⚠️ limited |
| Runtimes & Tools | ✅ | ✅ | ✅ |
| Installed Programs | ✅ registry | — | — |

---

## Output files

Reports are saved to the working directory by default:

```text
hwprofile_HOSTNAME_2026-03-15_09-22-41.txt
```

Override with `-o`:

```bash
hwprofile -o /tmp/report.txt
hwprofile -o \\server\share\reports\build.txt
```

A `.gitignore` is included that excludes `hwprofile_*.txt` automatically.

---

## Privacy

The binary makes **no network calls**. Everything runs locally.

Output files contain hardware serial numbers, MAC addresses, and installed
software lists. Use `--redact` before sharing output publicly — it strips
serials, MACs, and UUID-shaped values from the report.

---

## Contributing

Standard Rust workflow. `cargo test` before submitting a PR.

Platform-specific code lives in `src/collectors/` behind `#[cfg(target_os)]`
guards. Each collector implements the `Collector` trait — adding a new data
source means implementing the trait and registering it in `main.rs`.

---

## License

MIT
