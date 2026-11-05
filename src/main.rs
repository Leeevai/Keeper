use std::io::stdout;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use keeper::app::App;
use keeper::result::{StreakList, SummaryStats};
use keeper::settings::Settings;
use keeper::storage::Storage;
use keeper::ui::format_duration;
use ratatui::crossterm::cursor::SetCursorStyle;
use ratatui::crossterm::event::{self, DisableBracketedPaste, EnableBracketedPaste, Event};
use ratatui::crossterm::execute;

/// Keeper: a terminal typing tutor ported from keybr.com.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Directory for settings and results [default: $KEEPER_HOME or the platform data directory]
    #[arg(long, global = true, value_name = "DIR")]
    data_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Print a summary of your progress.
    Stats,
    /// Export all results as a JSON array.
    Export {
        /// The file to write.
        file: PathBuf,
    },
    /// Import results exported from Keeper or downloaded from keybr.com.
    Import {
        /// A JSON array or JSON lines file.
        file: PathBuf,
    },
    /// Print the directory where Keeper keeps its data.
    Path,
}

fn print_stats(storage: &Storage) -> Result<()> {
    let results = storage.load_results()?;
    let settings: Settings = storage.load_settings();
    let unit = settings.speed_unit;
    if results.is_empty() {
        println!("No lessons yet. Run `keeper` to start practicing.");
        return Ok(());
    }
    let all = SummaryStats::from_results(&results);
    let today = chrono::Local::now().date_naive();
    let today_stats =
        SummaryStats::from_results(results.iter().filter(|r| r.local_date() == today));
    println!(
        "All time:  {} lessons in {}",
        all.count,
        format_duration(all.time)
    );
    println!(
        "           top speed {}, average {}",
        unit.format(all.speed.max),
        unit.format(all.speed.avg)
    );
    println!(
        "           top accuracy {:.1}%, average {:.1}%",
        all.accuracy.max * 100.0,
        all.accuracy.avg * 100.0
    );
    if today_stats.count > 0 {
        println!(
            "Today:     {} lessons in {}",
            today_stats.count,
            format_duration(today_stats.time)
        );
        println!(
            "           top speed {}, average {}",
            unit.format(today_stats.speed.max),
            unit.format(today_stats.speed.avg)
        );
    }
    for streak in StreakList::find_longest(&results) {
        println!(
            "Longest streak at {:.0}% accuracy: {} lessons",
            streak.level * 100.0,
            streak.results.len()
        );
    }
    Ok(())
}

fn run(storage: Storage) -> Result<()> {
    let mut app = App::new(storage)?;
    let mut terminal = ratatui::try_init().context("cannot initialize the terminal")?;
    let _ = execute!(stdout(), EnableBracketedPaste);
    let result = event_loop(&mut terminal, &mut app);
    let _ = execute!(
        stdout(),
        DisableBracketedPaste,
        SetCursorStyle::DefaultUserShape
    );
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    // Key presses are timestamped as soon as they arrive, on their own
    // thread, so that drawing never delays the measured typing speed.
    let (tx, rx) = mpsc::channel::<(Event, Instant)>();
    thread::spawn(move || {
        while let Ok(event) = event::read() {
            if tx.send((event, Instant::now())).is_err() {
                break;
            }
        }
    });
    let mut bar_cursor = false;
    let mut dirty = true;
    loop {
        if dirty {
            let now = Instant::now();
            terminal.draw(|frame| app.render(frame, now))?;
            dirty = false;
        }
        let wants_bar = app.wants_bar_cursor();
        if wants_bar != bar_cursor {
            let style = if wants_bar {
                SetCursorStyle::SteadyBar
            } else {
                SetCursorStyle::DefaultUserShape
            };
            execute!(stdout(), style)?;
            bar_cursor = wants_bar;
        }
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok((event, at)) => {
                app.handle_event(event, at);
                // Handle everything that is already waiting before drawing again.
                while let Ok((event, at)) = rx.try_recv() {
                    app.handle_event(event, at);
                }
                dirty = true;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        let now = Instant::now();
        // Redraw without input only while something moves on the screen.
        dirty |= app.tick(now) | app.animating(now);
        if app.quit {
            break;
        }
    }
    Ok(())
}

fn main() {
    println!("keeper {}", env!("CARGO_PKG_VERSION"));
}
