# Pulse Monitor

A lightweight, native macOS system and process monitor built with Rust (Tauri) and Darwin/Mach kernel syscalls.

## Features

- **Activity Monitor Memory Logic:** Calculates total used memory (`App + Wired + Compressed`) via `host_statistics64` alongside kernel memory pressure (`Normal`, `Warning`, `Critical`).
- **10-Minute Timeline Scrubber:** Buffers the last 600 samples (1-second intervals) with an interactive sparkline scrubber to inspect past memory spikes.
- **Process Tree & Full Names:** Groups child processes under parent apps with full executable names resolved via `proc_pidpath`, physical memory footprint (`ri_phys_footprint`), CPU %, and per-second memory deltas.
- **Socket & Port Inspection:** Displays active listening TCP/UDP ports per process with a **Ports Only** filter and search by name, PID, or port.
- **Process Control:** Supports **Quit** (`SIGTERM`) and **Force Quit** (`SIGKILL`) with confirmation prompts.
- **Native macOS Window Behavior:** Closing the window keeps the sampler running in the background; clicking the Dock icon reopens the window with continuous history intact.

## Installation (macOS)

1. Download `Pulse Monitor_0.1.0_universal.dmg` from the [Releases](https://github.com/Maytid/pulse-monitor/releases) page.
2. Open the `.dmg` and drag **Pulse Monitor.app** into **Applications**.
3. Because the app is not notarized with an Apple Developer certificate, strip the macOS quarantine flag once in Terminal before launching:
   ```bash
   xattr -cr "/Applications/Pulse Monitor.app"
   ```

## Building from Source

Requires macOS, Rust (`cargo`), and the Tauri CLI (`cargo install tauri-cli`).

```bash
git clone [https://github.com/Maytid/pulse-monitor.git](https://github.com/Maytid/pulse-monitor.git)
cd pulse-monitor
cargo tauri build
```

## License

Licensed under the [GNU General Public License v3.0](LICENSE).