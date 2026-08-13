use std::fmt::Write as _;

use crate::{
    diagnosis::{diagnose, duration, overall},
    model::{Severity, Snapshot},
};

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";

fn color(severity: Severity) -> &'static str {
    match severity {
        Severity::Cool => "\x1b[32m",
        Severity::Warm => "\x1b[33m",
        Severity::Hot => "\x1b[31m",
    }
}

pub fn render(
    snapshot: &Snapshot,
    top: usize,
    selected: usize,
    width: usize,
    height: usize,
    frame: u64,
    show_processes: bool,
) -> String {
    let findings = diagnose(snapshot);
    let status = overall(&findings);
    let battery = match (snapshot.battery_percent, snapshot.charging) {
        (Some(p), Some(true)) => format!("{p}% charging"),
        (Some(p), _) => format!("{p}% battery"),
        _ => "battery unknown".into(),
    };
    let mut out = String::from("\x1b[H\x1b[2J");
    writeln!(
        out,
        "{BOLD}whyhot{RESET}  {}{}{RESET}  CPU {:.0}%  MEM {:.0}%  LOAD {:.1}  {battery}",
        color(status),
        status.label(),
        snapshot.cpu_percent,
        snapshot.memory_percent,
        snapshot.load[0]
    )
    .ok();
    for line in flame(status, snapshot.cpu_percent, frame) {
        writeln!(out, "{line}").ok();
    }
    writeln!(out, "{}", "─".repeat(width.min(100))).ok();
    writeln!(out, "{BOLD}Diagnosis{RESET}").ok();
    let mut diagnosis_lines = 1;
    for finding in &findings {
        let titles = wrap_words(&finding.title, width.saturating_sub(2).max(20));
        for (index, line) in titles.iter().enumerate() {
            let marker = if index == 0 { "● " } else { "  " };
            writeln!(out, "{}{marker}{line}{RESET}", color(finding.severity)).ok();
            diagnosis_lines += 1;
        }
        for line in wrap_words(&finding.detail, width.saturating_sub(4).max(20)) {
            writeln!(out, "    {line}").ok();
            diagnosis_lines += 1;
        }
    }
    if show_processes {
        writeln!(out, "\n{BOLD}Top applications{RESET}").ok();
        writeln!(
            out,
            "   {:<24} {:>7} {:>7} {:>8} {:>9} {:>5}",
            "APPLICATION", "CPU", "MEM", "I/O/s", "AGE", "PROCS"
        )
        .ok();
        let reserved = 14 + diagnosis_lines;
        let rows = top.min(height.saturating_sub(reserved)).max(1);
        for (index, app) in snapshot.applications.iter().take(rows).enumerate() {
            let marker = if index == selected { "›" } else { " " };
            writeln!(
                out,
                "{marker}  {:<24} {:>6.1}% {:>6.1}M {:>8} {:>9} {:>5}",
                truncate(&app.name, 24.min(width.saturating_sub(48))),
                app.cpu,
                app.memory_bytes as f64 / 1_048_576.0,
                io_rate(app.io_bytes_per_sec),
                duration(app.runtime_secs),
                app.pids.len()
            )
            .ok();
        }
        if let Some(app) = snapshot.applications.get(selected) {
            writeln!(out, "\n{BOLD}Selected{RESET}  {}", app.name).ok();
            writeln!(
                out,
                "    PIDs: {}",
                app.pids
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .ok();
        }
    } else {
        writeln!(
            out,
            "\n\x1b[2mPress p to inspect applications and their processes.\x1b[0m"
        )
        .ok();
    }
    write!(
        out,
        "\n\x1b[2m q quit · r refresh · p applications · j/k select · {} logical CPUs \x1b[0m",
        snapshot.logical_cpus
    )
    .ok();
    out
}

pub fn plain(snapshot: &Snapshot, top: usize) -> String {
    let findings = diagnose(snapshot);
    let mut out = format!(
        "whyhot: {} | CPU {:.0}% | memory {:.0}% | load {:.1}\n",
        overall(&findings).label(),
        snapshot.cpu_percent,
        snapshot.memory_percent,
        snapshot.load[0]
    );
    for finding in &findings {
        writeln!(out, "- {}: {}", finding.title, finding.detail).ok();
    }
    out.push_str("\nAPPLICATION                CPU    MEM     I/O/s      AGE  PROCS\n");
    for app in snapshot.applications.iter().take(top) {
        writeln!(
            out,
            "{:<24} {:>6.1}% {:>6.1}M {:>8} {:>8}  {}",
            truncate(&app.name, 24),
            app.cpu,
            app.memory_bytes as f64 / 1_048_576.0,
            io_rate(app.io_bytes_per_sec),
            duration(app.runtime_secs),
            app.pids.len()
        )
        .ok();
    }
    out
}

fn io_rate(value: u64) -> String {
    if value == 0 {
        "-".into()
    } else if value >= 1_073_741_824 {
        format!("{:.1}G", value as f64 / 1_073_741_824.0)
    } else if value >= 1_048_576 {
        format!("{:.1}M", value as f64 / 1_048_576.0)
    } else {
        format!("{:.0}K", value as f64 / 1024.0)
    }
}

fn truncate(value: &str, width: usize) -> String {
    let mut chars = value.chars();
    let mut result: String = chars.by_ref().take(width).collect();
    if chars.next().is_some() && width > 1 {
        result.pop();
        result.push('…');
    }
    result
}

fn wrap_words(value: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in value.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn flame(severity: Severity, cpu: f32, frame: u64) -> Vec<String> {
    let (outer, middle, core) = match severity {
        Severity::Hot => (
            "\x1b[38;2;255;62;48m",
            "\x1b[38;2;255;132;32m",
            "\x1b[38;2;255;226;92m",
        ),
        Severity::Warm => (
            "\x1b[38;2;255;120;38m",
            "\x1b[38;2;255;184;48m",
            "\x1b[38;2;255;235;120m",
        ),
        Severity::Cool => (
            "\x1b[38;2;54;143;255m",
            "\x1b[38;2;57;205;255m",
            "\x1b[38;2;155;240;255m",
        ),
    };
    let spark = match frame % 4 {
        0 => "        ·",
        1 => "      ·",
        2 => "         ·",
        _ => "       ˙",
    };
    let lean = if frame % 2 == 0 {
        "    ╭╯╰╮"
    } else {
        "     ╭╯╰╮"
    };
    let filled = (cpu.clamp(0.0, 100.0) / 10.0).round() as usize;
    let heat_bar = format!("{}{}", "■".repeat(filled), "·".repeat(10 - filled));
    let label = format!("{:.0}%", cpu);
    vec![
        format!("{outer}{spark}{RESET}"),
        format!("{outer}{lean}{RESET}"),
        format!("{outer}   ╭╯{middle}░▒{outer}╰╮{RESET}"),
        format!("{outer}  ╭╯{middle}▒▓▓▒{outer}╰╮{RESET}"),
        format!("{outer}  │{middle}▒▓{core}██{middle}▓▒{outer}│{RESET}"),
        format!("{outer}  ╰╮{middle}▓{core}████{middle}▓{outer}╭╯{RESET}"),
        format!(
            "{outer}   ╰━{core}██{outer}━╯{RESET}  {BOLD}{label}{RESET} {middle}{heat_bar}{RESET}"
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flame_reflects_heat_and_animates() {
        let first = flame(Severity::Hot, 93.0, 0);
        let second = flame(Severity::Hot, 93.0, 1);
        assert!(first.join("\n").contains("93%"));
        assert_ne!(first[0], second[0]);
    }

    #[test]
    fn problem_descriptions_wrap_without_losing_words() {
        let text = "Charging naturally adds heat, especially while the CPU is busy.";
        let lines = wrap_words(text, 20);
        assert!(lines.len() > 1);
        assert_eq!(lines.join(" "), text);
        assert!(lines.iter().all(|line| line.chars().count() <= 20));
    }
}
