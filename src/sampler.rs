use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, HashSet},
    process::Command,
    thread,
    time::{Duration, Instant},
};
use sysinfo::{ProcessesToUpdate, System};

use crate::model::{ApplicationRow, ProcessRow, Snapshot, ThermalState};

struct StaticInfo {
    battery_max_capacity: Option<u8>,
    battery_condition: Option<String>,
    external_displays: Option<usize>,
}

pub struct Sampler {
    system: System,
    static_info: StaticInfo,
    last_refresh: Instant,
    activity_streaks: HashMap<String, u8>,
    cpu_active_samples: u8,
}

impl Sampler {
    pub fn new() -> Self {
        let static_info = macos_static_info();
        let mut system = System::new_all();
        system.refresh_cpu_usage();
        system.refresh_processes(ProcessesToUpdate::All, true);
        let last_refresh = Instant::now();
        thread::sleep(Duration::from_millis(300));
        Self {
            system,
            static_info,
            last_refresh,
            activity_streaks: HashMap::new(),
            cpu_active_samples: 0,
        }
    }

    pub fn sample(&mut self) -> Snapshot {
        let now = Instant::now();
        let elapsed = now
            .duration_since(self.last_refresh)
            .as_secs_f64()
            .max(0.001);
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.system.refresh_processes(ProcessesToUpdate::All, true);
        let mut processes: Vec<_> = self
            .system
            .processes()
            .values()
            .map(|process| {
                let disk = process.disk_usage();
                let command = process
                    .cmd()
                    .iter()
                    .map(|part| part.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ");
                ProcessRow {
                    pid: process.pid().as_u32(),
                    parent_pid: process.parent().map(|pid| pid.as_u32()),
                    cpu: process.cpu_usage(),
                    memory_bytes: process.memory(),
                    io_bytes_per_sec: ((disk.read_bytes + disk.written_bytes) as f64 / elapsed)
                        as u64,
                    runtime_secs: process.run_time(),
                    name: process.name().to_string_lossy().into_owned(),
                    command,
                }
            })
            .collect();
        processes.sort_by(|a, b| {
            b.cpu
                .partial_cmp(&a.cpu)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.pid.cmp(&b.pid))
        });
        let uptime_secs = processes
            .iter()
            .map(|process| process.runtime_secs)
            // Some inaccessible/zombie macOS processes are reported as having
            // started at the Unix epoch. Exclude those impossible sentinel ages.
            .filter(|seconds| *seconds <= 365 * 86_400)
            .max()
            .unwrap_or(0);
        let mut applications = aggregate_applications(&processes);
        let current_names: HashSet<_> = applications.iter().map(|app| app.name.clone()).collect();
        self.activity_streaks
            .retain(|name, _| current_names.contains(name));
        for app in &mut applications {
            let active = app.cpu >= 20.0 || app.io_bytes_per_sec >= 10 * 1_048_576;
            let streak = self.activity_streaks.entry(app.name.clone()).or_default();
            *streak = if active { streak.saturating_add(1) } else { 0 };
            app.active_samples = *streak;
        }

        let total = self.system.total_memory();
        let memory_percent = if total == 0 {
            0.0
        } else {
            self.system.used_memory() as f32 * 100.0 / total as f32
        };
        let load = System::load_average();
        let (charging, battery_percent) = macos_battery();
        let cpu_percent = self.system.global_cpu_usage();
        self.cpu_active_samples = if cpu_percent >= 50.0 {
            self.cpu_active_samples.saturating_add(1)
        } else {
            0
        };
        self.last_refresh = now;
        Snapshot {
            cpu_percent,
            memory_percent,
            load: [load.one, load.five, load.fifteen],
            logical_cpus: self.system.cpus().len(),
            charging,
            battery_percent,
            battery_max_capacity: self.static_info.battery_max_capacity,
            battery_condition: self.static_info.battery_condition.clone(),
            external_displays: self.static_info.external_displays,
            thermal_state: macos_thermal_state(),
            uptime_secs,
            cpu_active_samples: self.cpu_active_samples,
            applications,
        }
    }
}

fn aggregate_applications(processes: &[ProcessRow]) -> Vec<ApplicationRow> {
    let by_pid: HashMap<_, _> = processes
        .iter()
        .map(|process| (process.pid, process))
        .collect();
    let mut groups: BTreeMap<String, ApplicationRow> = BTreeMap::new();
    for process in processes {
        let name = application_name(process, &by_pid);
        let app = groups.entry(name.clone()).or_insert(ApplicationRow {
            name,
            pids: Vec::new(),
            cpu: 0.0,
            memory_bytes: 0,
            io_bytes_per_sec: 0,
            runtime_secs: 0,
            active_samples: 0,
        });
        app.pids.push(process.pid);
        app.cpu += process.cpu;
        app.memory_bytes = app.memory_bytes.saturating_add(process.memory_bytes);
        app.io_bytes_per_sec = app
            .io_bytes_per_sec
            .saturating_add(process.io_bytes_per_sec);
        app.runtime_secs = app.runtime_secs.max(process.runtime_secs);
    }
    let mut applications: Vec<_> = groups.into_values().collect();
    for app in &mut applications {
        app.pids.sort_unstable();
    }
    applications.sort_by(|a, b| {
        b.cpu
            .partial_cmp(&a.cpu)
            .unwrap_or(Ordering::Equal)
            .then_with(|| b.io_bytes_per_sec.cmp(&a.io_bytes_per_sec))
            .then_with(|| a.name.cmp(&b.name))
    });
    applications
}

fn application_name(process: &ProcessRow, by_pid: &HashMap<u32, &ProcessRow>) -> String {
    let value = format!("{} {}", process.name, process.command);
    let lower = value.to_lowercase();
    if lower.contains("google chrome") {
        return "Google Chrome".into();
    }
    if lower.contains("firefox") {
        return "Firefox".into();
    }
    if lower.contains("webkit") {
        let mut parent = process.parent_pid;
        for _ in 0..8 {
            let Some(ancestor) = parent.and_then(|pid| by_pid.get(&pid).copied()) else {
                break;
            };
            if let Some(name) = app_bundle_name(&ancestor.command) {
                return name;
            }
            parent = ancestor.parent_pid;
        }
        return "Safari/WebKit".into();
    }
    if let Some(name) = app_bundle_name(&process.command) {
        return name;
    }
    process.name.clone()
}

fn app_bundle_name(command: &str) -> Option<String> {
    let end = command.find(".app/").or_else(|| command.find(".app"))?;
    command[..end]
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}

fn macos_static_info() -> StaticInfo {
    let Ok(output) = Command::new("system_profiler")
        .args([
            "SPDisplaysDataType",
            "SPPowerDataType",
            "-detailLevel",
            "mini",
        ])
        .output()
    else {
        return StaticInfo {
            battery_max_capacity: None,
            battery_condition: None,
            external_displays: None,
        };
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let lower = text.to_lowercase();
    let battery_max_capacity = line_value(&text, "Maximum Capacity:")
        .and_then(|value| value.trim_end_matches('%').parse().ok());
    let battery_condition = line_value(&text, "Condition:").map(str::to_owned);
    let display_count = lower
        .lines()
        .filter(|line| line.trim_start().starts_with("resolution:"))
        .count();
    let built_in_count = lower
        .lines()
        .filter(|line| line.contains("display type:") && line.contains("built-in"))
        .count();
    StaticInfo {
        battery_max_capacity,
        battery_condition,
        external_displays: Some(display_count.saturating_sub(built_in_count)),
    }
}

fn line_value<'a>(text: &'a str, label: &str) -> Option<&'a str> {
    text.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(label).map(str::trim))
}

fn macos_thermal_state() -> Option<ThermalState> {
    let output = Command::new("pmset").args(["-g", "therm"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    parse_thermal_state(&String::from_utf8_lossy(&output.stdout))
}

fn parse_thermal_state(text: &str) -> Option<ThermalState> {
    let cpu_limit = numeric_setting(text, "CPU_Speed_Limit");
    let scheduler_limit = numeric_setting(text, "Scheduler_Limit");
    let thermal_level = numeric_setting(text, "Thermal_Level");
    if cpu_limit.is_some_and(|value| value < 100) {
        Some(ThermalState::Throttled)
    } else if scheduler_limit.is_some_and(|value| value < 100)
        || thermal_level.is_some_and(|value| value > 0)
    {
        Some(ThermalState::Warning)
    } else if cpu_limit.is_some() || scheduler_limit.is_some() || thermal_level.is_some() {
        Some(ThermalState::Normal)
    } else {
        None
    }
}

fn numeric_setting(text: &str, key: &str) -> Option<u32> {
    text.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name.trim() == key)
            .then(|| value.trim().parse().ok())
            .flatten()
    })
}

fn macos_battery() -> (Option<bool>, Option<u8>) {
    let Ok(output) = Command::new("pmset").args(["-g", "batt"]).output() else {
        return (None, None);
    };
    let text = String::from_utf8_lossy(&output.stdout).to_lowercase();
    let charging = if text.contains("discharging") {
        Some(false)
    } else if text.contains("charging") || text.contains("charged") {
        Some(true)
    } else {
        None
    };
    let percent = text.split_whitespace().find_map(|part| {
        part.contains('%')
            .then(|| {
                part.trim_matches(|c: char| !c.is_ascii_digit())
                    .parse()
                    .ok()
            })
            .flatten()
    });
    (charging, percent)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(
        pid: u32,
        parent_pid: Option<u32>,
        name: &str,
        command: &str,
        cpu: f32,
    ) -> ProcessRow {
        ProcessRow {
            pid,
            parent_pid,
            cpu,
            memory_bytes: 10,
            io_bytes_per_sec: 20,
            runtime_secs: 30,
            name: name.into(),
            command: command.into(),
        }
    }

    #[test]
    fn detects_thermal_throttling() {
        let text = "CPU_Speed_Limit = 65\nScheduler_Limit = 100\nThermal_Level = 0";
        assert_eq!(parse_thermal_state(text), Some(ThermalState::Throttled));
    }

    #[test]
    fn detects_normal_thermal_state() {
        let text = "CPU_Speed_Limit = 100\nScheduler_Limit = 100\nThermal_Level = 0";
        assert_eq!(parse_thermal_state(text), Some(ThermalState::Normal));
    }

    #[test]
    fn groups_application_helpers_and_sums_evidence() {
        let rows = vec![
            process(
                10,
                None,
                "Google Chrome",
                "/Applications/Google Chrome.app/Chrome",
                12.0,
            ),
            process(
                11,
                Some(10),
                "Google Chrome Helper",
                "/Applications/Google Chrome.app/Helper",
                34.0,
            ),
        ];
        let apps = aggregate_applications(&rows);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Google Chrome");
        assert_eq!(apps[0].pids, vec![10, 11]);
        assert_eq!(apps[0].cpu, 46.0);
        assert_eq!(apps[0].io_bytes_per_sec, 40);
    }
}
