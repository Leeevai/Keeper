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

fn main() {
    println!("keeper {}", env!("CARGO_PKG_VERSION"));
}
