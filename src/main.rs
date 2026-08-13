mod cli;
mod diagnosis;
mod model;
mod sampler;
mod terminal;
mod ui;

use std::{
    io::{self, IsTerminal, Read, Write},
    thread,
    time::{Duration, Instant},
};

use cli::{Action, Config};
use sampler::Sampler;

fn main() {
    if let Err(error) = run() {
        eprintln!("whyhot: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    match cli::parse().map_err(io::Error::other)? {
        Action::Help => print!("{}", cli::HELP),
        Action::Version => println!("whyhot {}", env!("CARGO_PKG_VERSION")),
        Action::Run(config) => execute(config)?,
    }
    Ok(())
}

fn execute(config: Config) -> io::Result<()> {
    let mut sampler = Sampler::new();
    if config.once || !io::stdout().is_terminal() || !io::stdin().is_terminal() {
        print!("{}", ui::plain(&sampler.sample(), config.top));
        return Ok(());
    }
    let _terminal = terminal::RawTerminal::enter()?;
    let mut stdout = io::stdout().lock();
    let mut selected = 0;
    let mut frame = 0;
    let mut show_processes = false;
    let mut snapshot = sampler.sample();
    let mut sampled_at = Instant::now();
    loop {
        let (width, height) = terminal::size();
        write!(
            stdout,
            "{}",
            ui::render(
                &snapshot,
                config.top,
                selected,
                width,
                height,
                frame,
                show_processes,
            )
        )?;
        stdout.flush()?;
        let deadline = sampled_at + config.interval;
        let mut next_frame = Instant::now() + Duration::from_millis(300);
        loop {
            let mut bytes = [0_u8; 3];
            let read = io::stdin().read(&mut bytes)?;
            if read > 0 {
                match &bytes[..read] {
                    b"q" | b"Q" => return Ok(()),
                    b"j" | b"\x1b[B" => {
                        selected = (selected + 1).min(
                            config
                                .top
                                .min(snapshot.applications.len())
                                .saturating_sub(1),
                        )
                    }
                    b"k" | b"\x1b[A" => selected = selected.saturating_sub(1),
                    b"p" | b"P" => show_processes = !show_processes,
                    b"r" | b"R" => break,
                    _ => {}
                }
                let (width, height) = terminal::size();
                write!(
                    stdout,
                    "{}",
                    ui::render(
                        &snapshot,
                        config.top,
                        selected,
                        width,
                        height,
                        frame,
                        show_processes,
                    )
                )?;
                stdout.flush()?;
            }
            if Instant::now() >= deadline {
                break;
            }
            if Instant::now() >= next_frame {
                frame = frame.wrapping_add(1);
                let (width, height) = terminal::size();
                write!(
                    stdout,
                    "{}",
                    ui::render(
                        &snapshot,
                        config.top,
                        selected,
                        width,
                        height,
                        frame,
                        show_processes,
                    )
                )?;
                stdout.flush()?;
                next_frame = Instant::now() + Duration::from_millis(300);
            }
            thread::sleep(Duration::from_millis(40));
        }
        let selected_name = snapshot
            .applications
            .get(selected)
            .map(|app| app.name.clone());
        snapshot = sampler.sample();
        selected = selected_name
            .and_then(|name| {
                snapshot
                    .applications
                    .iter()
                    .position(|app| app.name == name)
            })
            .unwrap_or_else(|| selected.min(snapshot.applications.len().saturating_sub(1)));
        sampled_at = Instant::now();
    }
}
