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

fn main() {
    println!("keeper {}", env!("CARGO_PKG_VERSION"));
}
