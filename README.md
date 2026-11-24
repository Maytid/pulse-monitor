# Pulse Monitor

A lightweight, macOS-only process and memory monitor built with Rust (Tauri) and native Darwin/Mach kernel APIs. Built to pair Activity Monitor's exact memory readings with live listening ports, per-second memory deltas, and a 10-minute timeline scrubber in one place.

## What It Does

- **Matches Activity Monitor's Memory Readings** — reads `App + Wired + Compressed` memory directly from `host_statistics64` along with macOS kernel memory pressure (`Normal`, `Warning`, `Critical`).
- **10-Minute History Scrubber** — buffers the last 600 ticks (1s intervals) with a sparkline so you can scrub back and see what caused a memory spike.
- **Process Trees & Memory Deltas** — groups child processes under their parent app, resolves full executable names, and tracks per-second RAM changes (`+`/`-`).
- **Port & Socket Visibility** — shows active listening TCP/UDP ports next to each process, with a **Ports Only** toggle and instant filtering by name, PID, or port.
- **Quit & Force Quit** — send `SIGTERM` or `SIGKILL` directly from the process list.
- **Native Dock Behavior** — clicking the `x` button hides the window while keeping the background sampler running; clicking the Dock icon brings it back with your history saved.

## Install

1. Download `Pulse Monitor_0.1.0_universal.dmg` from [Releases](https://github.com/Maytid/pulse-monitor/releases) (runs natively on both Apple Silicon and Intel Macs).
2. Open the `.dmg` and move **Pulse Monitor.app** into **Applications**.
3. Since the app isn't signed with a paid Apple Developer ID, macOS will block it on first download. Bypass that in Terminal before launching:
   ```bash
   xattr -cr "/Applications/Pulse Monitor.app"
   ```

## Build From Source

Because the backend calls XNU/Mach syscalls directly, this only compiles and runs on macOS. You'll need Rust and the Tauri CLI (`cargo install tauri-cli`):

```bash
git clone [https://github.com/Maytid/pulse-monitor.git](https://github.com/Maytid/pulse-monitor.git)
cd pulse-monitor
cargo tauri build
```

## License

[GPL-3.0](LICENSE)