use std::{env, time::Duration};

#[derive(Debug, PartialEq)]
pub struct Config {
    pub once: bool,
    pub interval: Duration,
    pub top: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            once: false,
            interval: Duration::from_secs(2),
            top: 10,
        }
    }
}

pub enum Action {
    Run(Config),
    Help,
    Version,
}

pub fn parse() -> Result<Action, String> {
    parse_args(env::args().skip(1))
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Action, String> {
    let mut config = Config::default();
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--once" | "-1" => config.once = true,
            "--help" | "-h" => return Ok(Action::Help),
            "--version" | "-V" => return Ok(Action::Version),
            "--interval" | "-i" => {
                let value = args.next().ok_or("--interval requires seconds")?;
                let seconds: f64 = value.parse().map_err(|_| "interval must be a number")?;
                if !(0.25..=60.0).contains(&seconds) {
                    return Err("interval must be between 0.25 and 60 seconds".into());
                }
                config.interval = Duration::from_secs_f64(seconds);
            }
            "--top" | "-n" => {
                let value = args.next().ok_or("--top requires a count")?;
                config.top = value.parse().map_err(|_| "top must be an integer")?;
                if !(1..=50).contains(&config.top) {
                    return Err("top must be between 1 and 50".into());
                }
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    Ok(Action::Run(config))
}

pub const HELP: &str = "whyhot — explain what is heating your Mac

Usage: whyhot [OPTIONS]

Options:
  -1, --once             Print one diagnosis and exit
  -i, --interval SEC     Refresh interval, 0.25–60 [default: 2]
  -n, --top COUNT        Applications to show, 1–50 [default: 10]
  -h, --help             Print help
  -V, --version          Print version

TUI keys: q quit · r refresh · p application list · ↑/↓ or j/k select
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_snapshot_options() {
        let args = ["--once", "--top", "7", "--interval", "0.5"];
        let action = parse_args(args.into_iter().map(String::from)).unwrap();
        let Action::Run(config) = action else {
            panic!("expected run")
        };
        assert_eq!(config.top, 7);
        assert!(config.once);
        assert_eq!(config.interval, Duration::from_millis(500));
    }
}
