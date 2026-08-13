#[derive(Clone, Debug)]
pub struct ProcessRow {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub cpu: f32,
    pub memory_bytes: u64,
    pub io_bytes_per_sec: u64,
    pub runtime_secs: u64,
    pub name: String,
    pub command: String,
}

#[derive(Clone, Debug)]
pub struct ApplicationRow {
    pub name: String,
    pub pids: Vec<u32>,
    pub cpu: f32,
    pub memory_bytes: u64,
    pub io_bytes_per_sec: u64,
    pub runtime_secs: u64,
    pub active_samples: u8,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub cpu_percent: f32,
    pub memory_percent: f32,
    pub load: [f64; 3],
    pub logical_cpus: usize,
    pub charging: Option<bool>,
    pub battery_percent: Option<u8>,
    pub battery_max_capacity: Option<u8>,
    pub battery_condition: Option<String>,
    pub external_displays: Option<usize>,
    pub thermal_state: Option<ThermalState>,
    pub uptime_secs: u64,
    pub cpu_active_samples: u8,
    pub applications: Vec<ApplicationRow>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThermalState {
    Normal,
    Warning,
    Throttled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Cool,
    Warm,
    Hot,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cool => "COOL",
            Self::Warm => "WARM",
            Self::Hot => "HOT",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Finding {
    pub severity: Severity,
    pub title: String,
    pub detail: String,
}
