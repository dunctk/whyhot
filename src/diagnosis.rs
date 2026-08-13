use crate::model::{ApplicationRow, Finding, Severity, Snapshot, ThermalState};

pub fn diagnose(snapshot: &Snapshot) -> Vec<Finding> {
    let mut findings = Vec::new();
    for app in snapshot
        .applications
        .iter()
        .filter(|app| {
            app.cpu >= 50.0
                || app.io_bytes_per_sec >= 10 * 1_048_576
                || (app.cpu >= 20.0 && workload(app).is_some())
        })
        .take(5)
    {
        findings.push(application_finding(app));
    }
    if snapshot.cpu_percent >= 80.0 {
        let sustained = snapshot.cpu_active_samples >= 3;
        findings.push(Finding {
            severity: if sustained {
                Severity::Hot
            } else {
                Severity::Warm
            },
            title: format!("Overall CPU load is high{}", observing(sustained)),
            detail: sample_detail(
                snapshot.cpu_active_samples,
                "Sustained computation is the most likely source of heat.",
            ),
        });
    } else if snapshot.cpu_percent >= 50.0 {
        findings.push(Finding {
            severity: Severity::Warm,
            title: format!(
                "Overall CPU load is elevated{}",
                observing(snapshot.cpu_active_samples >= 3)
            ),
            detail: sample_detail(
                snapshot.cpu_active_samples,
                "Check the leading applications if this persists.",
            ),
        });
    }
    if snapshot.charging == Some(true) {
        findings.push(Finding {
            severity: Severity::Warm,
            title: "Battery is charging".into(),
            detail: "Charging naturally adds heat, especially while the CPU is busy.".into(),
        });
    }
    match snapshot.thermal_state {
        Some(ThermalState::Throttled) => findings.push(Finding {
            severity: Severity::Hot,
            title: "macOS is thermally throttling the CPU".into(),
            detail: "The system has reduced CPU speed to control temperature. Lower the workload, disconnect unnecessary peripherals, improve airflow, and allow time to cool.".into(),
        }),
        Some(ThermalState::Warning) => findings.push(Finding {
            severity: Severity::Hot,
            title: "macOS reports thermal pressure".into(),
            detail: "The machine is actively managing excess heat. Reduce intensive work and check that its vents and surrounding air are unobstructed.".into(),
        }),
        _ => {}
    }
    if snapshot.memory_percent >= 90.0 {
        findings.push(Finding {
            severity: Severity::Warm,
            title: "Memory use is very high".into(),
            detail: "Memory pressure and swapping can increase power use.".into(),
        });
    }
    if snapshot
        .battery_max_capacity
        .is_some_and(|capacity| capacity < 80)
    {
        findings.push(Finding {
            severity: Severity::Warm,
            title: "Battery health is degraded".into(),
            detail: format!(
                "Maximum capacity is {}%. An ageing or damaged battery can run warmer, especially while charging. Check Battery settings or Apple Diagnostics.",
                snapshot.battery_max_capacity.unwrap_or_default()
            ),
        });
    }
    if snapshot
        .battery_condition
        .as_deref()
        .is_some_and(|condition| !condition.eq_ignore_ascii_case("normal"))
    {
        findings.push(Finding {
            severity: Severity::Warm,
            title: "macOS reports a battery service condition".into(),
            detail: format!(
                "Battery condition: {}. Avoid sustained heavy use while charging and arrange a battery check.",
                snapshot.battery_condition.as_deref().unwrap_or("unknown")
            ),
        });
    }
    if snapshot.external_displays.is_some_and(|count| count > 0) {
        let count = snapshot.external_displays.unwrap_or_default();
        findings.push(Finding {
            severity: Severity::Cool,
            title: format!("{count} external display{} connected", if count == 1 { "" } else { "s" }),
            detail: "External displays increase display-engine and WindowServer work. High resolution, HDR, scaling, and high refresh rates add more load.".into(),
        });
    }
    if snapshot.uptime_secs >= 14 * 86_400 {
        findings.push(Finding {
            severity: Severity::Cool,
            title: format!("Mac has been running for {}", duration(snapshot.uptime_secs)),
            detail: "Long uptime is not inherently harmful, but restarting can clear stuck background services if the heat has no other explanation.".into(),
        });
    }
    if !findings
        .iter()
        .any(|finding| finding.severity != Severity::Cool)
    {
        findings.push(Finding {
            severity: Severity::Cool,
            title: "No obvious software heat source".into(),
            detail: "Software cannot sense blocked vents, a soft surface, direct sun, or high room temperature. Move the Mac to a hard ventilated surface, disconnect unneeded docks, and check whether it cools.".into(),
        });
    }
    findings.sort_by_key(|f| match f.severity {
        Severity::Hot => 0,
        Severity::Warm => 1,
        Severity::Cool => 2,
    });
    findings
}

fn application_finding(app: &ApplicationRow) -> Finding {
    let cpu_busy = app.cpu >= 50.0;
    let io_busy = app.io_bytes_per_sec >= 10 * 1_048_576;
    let sustained = app.active_samples >= 3;
    let title = match (cpu_busy, io_busy, workload(app)) {
        (true, true, _) => format!("{} is using CPU and disk", app.name),
        (true, false, _) => format!("{} is using {:.0}% CPU", app.name, app.cpu),
        (false, true, _) => format!("{} is doing heavy disk I/O", app.name),
        (_, _, Some((title, _))) => title.into(),
        _ => format!("{} is active", app.name),
    };
    let mut evidence = vec![format!(
        "Across {} process{} ({}): {:.0}% CPU",
        app.pids.len(),
        if app.pids.len() == 1 { "" } else { "es" },
        pid_summary(&app.pids),
        app.cpu
    )];
    if io_busy {
        evidence.push(format!("about {}/s disk I/O", bytes(app.io_bytes_per_sec)));
    }
    evidence.push(format!("running {}", duration(app.runtime_secs)));
    let mut detail = format!("{}. ", evidence.join(" · "));
    if !sustained {
        detail.push_str(&sample_detail(app.active_samples, ""));
        detail.push(' ');
    }
    if let Some((_, explanation)) = workload(app) {
        detail.push_str(explanation);
        detail.push(' ');
    }
    if app.name.to_lowercase().contains("kernel_task") {
        detail.push_str("Do not kill it; reduce other workloads and let the Mac cool.");
    } else {
        detail.push_str("Quit the application normally to confirm whether it is the cause.");
    }
    Finding {
        severity: if sustained && (app.cpu >= 90.0 || app.io_bytes_per_sec >= 50 * 1_048_576) {
            Severity::Hot
        } else {
            Severity::Warm
        },
        title: format!("{title}{}", observing(sustained)),
        detail,
    }
}

fn sample_detail(samples: u8, followup: &str) -> String {
    if samples >= 3 {
        followup.into()
    } else {
        format!(
            "Observed for {} consecutive sample{}; watching for sustained activity.{}{}",
            samples.max(1),
            if samples == 1 { "" } else { "s" },
            if followup.is_empty() { "" } else { " " },
            followup
        )
    }
}

fn observing(sustained: bool) -> &'static str {
    if sustained { "" } else { " — observing" }
}

fn pid_summary(pids: &[u32]) -> String {
    let mut result = pids
        .iter()
        .take(6)
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    if pids.len() > 6 {
        result.push_str(", …");
    }
    format!("PID{} {result}", if pids.len() == 1 { "" } else { "s" })
}

fn workload(app: &ApplicationRow) -> Option<(&'static str, &'static str)> {
    let value = app.name.to_lowercase();
    let matches = |names: &[&str]| names.iter().any(|name| value.contains(name));
    if matches(&["kernel_task"]) {
        Some((
            "Thermal management is active",
            "High kernel_task CPU often means macOS is deliberately limiting other work because the hardware is hot; kernel_task itself is not the cause.",
        ))
    } else if matches(&["mds", "mdworker", "corespotlight"]) {
        Some((
            "Spotlight indexing is active",
            "Spotlight is scanning files. This commonly happens after an update, migration, or large file change and should settle when indexing finishes.",
        ))
    } else if matches(&["photoanalysisd", "photolibraryd", "mediaanalysisd"]) {
        Some((
            "Photos analysis is active",
            "Photos is indexing faces, objects, or media. It often runs after importing a library or while the Mac is plugged in.",
        ))
    } else if matches(&["backupd", "time machine"]) {
        Some((
            "A backup is active",
            "Time Machine or another backup job can use CPU, storage, and network together. Let it finish or pause it temporarily.",
        ))
    } else if matches(&["softwareupdated", "installd", "osinstall"]) {
        Some((
            "A software update is active",
            "Downloading, verifying, and installing updates can temporarily produce substantial heat.",
        ))
    } else if matches(&["bird", "cloudd", "icloud"]) {
        Some((
            "iCloud syncing is active",
            "iCloud is reconciling or transferring files. Large initial syncs can keep storage, networking, and CPU busy.",
        ))
    } else if matches(&["windowserver"]) {
        Some((
            "Display compositing is busy",
            "WindowServer load often rises with external displays, scaled resolutions, HDR, screen sharing, animation, or many changing windows.",
        ))
    } else if matches(&["vtencoder", "ffmpeg", "handbrake", "com.apple.videotoolbox"]) {
        Some((
            "Video encoding is active",
            "Video export, transcoding, or screen recording can heavily use media engines and the GPU as well as CPU.",
        ))
    } else if matches(&[
        "qemu",
        "virtualization",
        "parallels",
        "vmware",
        "docker desktop",
    ]) {
        Some((
            "A virtual machine is active",
            "Virtual machines and containers can sustain CPU, memory, disk, and network load even when their windows are hidden.",
        ))
    } else if matches(&["safari", "webkit", "chrome", "firefox", "chromium"]) {
        Some((
            "Browser content is busy",
            "A tab may be running animation, video, WebGL, JavaScript, or a call. Use the browser task manager or close tabs to isolate it.",
        ))
    } else if matches(&["syncthing", "rclone", "dropbox", "onedrive"]) {
        Some((
            "File syncing is active",
            "Scanning, hashing, encrypting, and transferring many files can heat the CPU and storage. Pause the sync to confirm.",
        ))
    } else {
        None
    }
}

fn bytes(value: u64) -> String {
    if value >= 1_073_741_824 {
        format!("{:.1} GiB", value as f64 / 1_073_741_824.0)
    } else {
        format!("{:.1} MiB", value as f64 / 1_048_576.0)
    }
}

pub fn overall(findings: &[Finding]) -> Severity {
    findings
        .iter()
        .map(|f| f.severity)
        .min_by_key(|s| match s {
            Severity::Hot => 0,
            Severity::Warm => 1,
            Severity::Cool => 2,
        })
        .unwrap_or(Severity::Cool)
}

pub fn duration(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = seconds % 86_400 / 3_600;
    let minutes = seconds % 3_600 / 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(cpu: f32) -> Snapshot {
        Snapshot {
            cpu_percent: cpu,
            memory_percent: 20.0,
            load: [0.0; 3],
            logical_cpus: 8,
            charging: None,
            battery_percent: None,
            battery_max_capacity: None,
            battery_condition: None,
            external_displays: Some(0),
            thermal_state: Some(ThermalState::Normal),
            uptime_secs: 3_600,
            cpu_active_samples: 0,
            applications: vec![],
        }
    }

    #[test]
    fn quiet_machine_is_cool() {
        assert_eq!(overall(&diagnose(&snapshot(10.0))), Severity::Cool);
    }

    #[test]
    fn sustained_runaway_application_is_hot_and_identified() {
        let mut value = snapshot(95.0);
        value.cpu_active_samples = 3;
        value.applications.push(ApplicationRow {
            name: "spinner".into(),
            pids: vec![42, 43],
            cpu: 99.0,
            memory_bytes: 0,
            io_bytes_per_sec: 60 * 1_048_576,
            runtime_secs: 500,
            active_samples: 3,
        });
        let findings = diagnose(&value);
        assert_eq!(overall(&findings), Severity::Hot);
        assert!(findings.iter().any(|f| f.title.contains("spinner")));
        assert_eq!(
            findings
                .iter()
                .filter(|f| f.title.contains("spinner"))
                .count(),
            1
        );
    }

    #[test]
    fn explains_known_spotlight_workload() {
        let mut value = snapshot(35.0);
        value.applications.push(ApplicationRow {
            name: "mdworker_shared".into(),
            pids: vec![7],
            cpu: 30.0,
            memory_bytes: 0,
            io_bytes_per_sec: 0,
            runtime_secs: 60,
            active_samples: 1,
        });
        assert!(
            diagnose(&value)
                .iter()
                .any(|finding| finding.title.contains("Spotlight"))
        );
    }

    #[test]
    fn thermal_throttling_is_hot() {
        let mut value = snapshot(20.0);
        value.thermal_state = Some(ThermalState::Throttled);
        assert_eq!(overall(&diagnose(&value)), Severity::Hot);
    }

    #[test]
    fn first_extreme_sample_is_observing_not_hot() {
        let mut value = snapshot(95.0);
        value.cpu_active_samples = 1;
        value.applications.push(ApplicationRow {
            name: "spinner".into(),
            pids: vec![42],
            cpu: 99.0,
            memory_bytes: 0,
            io_bytes_per_sec: 0,
            runtime_secs: 5,
            active_samples: 1,
        });
        let findings = diagnose(&value);
        assert_eq!(overall(&findings), Severity::Warm);
        assert!(
            findings
                .iter()
                .any(|finding| finding.title.contains("observing"))
        );
    }
}
