# whyhot

`whyhot` is a deterministic macOS terminal diagnostic that answers “why is my Mac hot?” It combines process activity with macOS power, thermal, battery, display, and uptime signals. Its animated flame changes colour with the diagnosis and intensity with CPU load.

![Rust 1.85+](https://img.shields.io/badge/Rust-1.85%2B-orange)

## Install

`whyhot` supports Apple Silicon and Intel Macs. Choose whichever installer you already use.

### Run with npx

No permanent install and no Rust toolchain required:

```sh
npx whyhot
```

### Install with npm

```sh
npm install --global whyhot
whyhot
```

The npm package bundles both native macOS binaries. It has no runtime dependencies, install scripts, telemetry, or binary downloads.

### Install with Cargo

```sh
cargo install whyhot --locked
whyhot
```

This builds from source and requires a Rust toolchain.

### Install from Git

```sh
cargo install --git https://github.com/dunctk/whyhot --locked
```

### Install from a local checkout

```sh
cargo install --path . --locked
```

## Use

Launch the live terminal UI:

```sh
whyhot
```

The application list is hidden by default so the full diagnosis stays readable. Press `p` to show or hide it, then use `j`/`k` or the arrow keys to inspect grouped processes.

Print a stable snapshot for scripts or support requests:

```sh
whyhot --once --top 15
```

The UI is read-only. It suggests a PID to inspect or stop, but never kills a process. CPU figures are sampled locally using macOS process counters; battery state comes from `pmset`. No data leaves the Mac.

## Rules

- A process at 50% CPU is marked warm; 90% is hot.
- Overall CPU at 50% is warm; 80% is hot.
- Per-process disk activity above 10 MiB/s is warm; 50 MiB/s is hot.
- Charging and memory use above 90% are noted as contributing factors.
- Thermal pressure or CPU throttling reported by macOS is hot.
- Battery service conditions and capacity below 80% are flagged.
- External displays and long uptime are reported as possible contributors.
- Spotlight, Photos, backups, updates, iCloud, browsers, video encoders, file sync, virtual machines, WindowServer, and `kernel_task` receive workload-specific explanations.
- Applications are sorted by descending CPU, then disk activity and name; PIDs inside each group are sorted numerically, so equal samples have stable output.
- Helper processes are grouped under their owning application where possible.
- CPU, disk, and known-workload evidence is combined into one finding per application.
- A high reading must persist for three samples before it can be classified as hot; earlier readings are labelled “observing.”

GPU load is inferred from known browser, video, VM, and display workloads because macOS does not expose reliable per-process GPU energy metrics through an unprivileged stable command. Airflow, soft surfaces, direct sunlight, and room temperature are included in the fallback checklist because they cannot be measured in software.

## Development

```sh
cargo test --locked
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

## Releasing

Update the Rust and npm package versions together, run the development checks plus `cargo package --locked`, then publish a GitHub release. Maintainers can publish through the manual **Publish to crates.io** workflow and the OIDC-authenticated **Publish to npm** workflow.

## License

MIT
