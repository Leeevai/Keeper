//! The profile screen: statistics and charts of your progress.

use std::collections::{BTreeSet, HashMap};

use chrono::{Datelike, Duration, Local, NaiveDate};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Axis, Bar, BarChart, BarGroup, Block, BorderType, Borders, Chart, Dataset, GraphType,
    Paragraph, Widget,
};

use super::RenderContext;
use crate::keyboard::{Keyboard, Layout as KeyboardLayout};
use crate::lesson::{Target, shared_model};
use crate::result::{KeyStatsMap, StreakList, SummaryStats, TypingResult, time_to_speed};
use crate::settings::{Settings, SpeedUnit};
use crate::stats::{
    CharClass, Distribution, key_usage, practice_days, progress_overview, smooth_series, week_start,
};
use crate::theme::Rgb;
use crate::ui::keyboard::KeyboardView;
use crate::ui::{Styles, format_duration, thousands};

/// What a key press on the profile screen asks the app to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileAction {
    None,
    Back,
    Export,
    Reset,
}

pub struct Profile {
    pub scroll: u16,
    /// From 0 to 1.
    pub smoothness: f64,
    pub char_class: CharClass,
    /// The layout whose key statistics are shown.
    pub layout: Option<String>,
    /// The key shown in the key speed chart.
    pub key_index: usize,
    /// Waiting for the reset to be confirmed.
    pub confirm_reset: bool,
    /// The height of the page when it was last drawn.
    page_height: std::cell::Cell<u16>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            scroll: 0,
            smoothness: 0.5,
            char_class: CharClass::Letters,
            layout: None,
            key_index: 0,
            confirm_reset: false,
            page_height: std::cell::Cell::new(0),
        }
    }
}

const WIDTH: u16 = 112;

/// A titled block of the page: its title, height and how to draw it.
type Section<'a> = (String, u16, Box<dyn Fn(Rect, &mut Buffer) + 'a>);

/// Layouts that have results, most used first.
fn layouts_in(results: &[TypingResult]) -> Vec<String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for r in results {
        *counts.entry(r.layout.as_str()).or_default() += 1;
    }
    let mut layouts: Vec<(&str, usize)> = counts.into_iter().collect();
    layouts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    layouts.into_iter().map(|(l, _)| l.to_string()).collect()
}

fn summary<'a>(
    s: &Styles,
    unit: SpeedUnit,
    title: &'a str,
    stats: &SummaryStats,
    goal: Option<crate::result::DailyGoal>,
) -> Paragraph<'a> {
    let row = |name: &str, value: String| {
        Line::from(vec![
            Span::styled(format!("{name:<18}"), s.name),
            Span::styled(value, s.value),
        ])
    };
    let none = || "-".to_string();
    let has = stats.count > 0;
    let mut lines = vec![
        Line::from(Span::styled(title, s.text.add_modifier(Modifier::BOLD))),
        row("Lessons", thousands(stats.count as f64)),
        row("Time", format_duration(stats.time)),
        row(
            "Top speed",
            if has {
                unit.format(stats.speed.max)
            } else {
                none()
            },
        ),
        row(
            "Average speed",
            if has {
                unit.format(stats.speed.avg)
            } else {
                none()
            },
        ),
        row(
            "Top accuracy",
            if has {
                format!("{:.1}%", stats.accuracy.max * 100.0)
            } else {
                none()
            },
        ),
        row(
            "Average accuracy",
            if has {
                format!("{:.1}%", stats.accuracy.avg * 100.0)
            } else {
                none()
            },
        ),
    ];
    if let Some(goal) = goal {
        lines.push(row(
            "Daily goal",
            format!("{:.0}% of {} minutes", goal.value * 100.0, goal.goal),
        ));
    }
    Paragraph::new(lines)
}

fn streak_table<'a>(
    s: &Styles,
    unit: SpeedUnit,
    streaks: &[crate::result::Streak],
) -> Paragraph<'a> {
    if streaks.is_empty() {
        return Paragraph::new(Span::styled(
            "You don't have any accuracy streaks. Consider completing a lesson with the highest accuracy \
             possible, regardless of typing speed.",
            s.name,
        ))
        .wrap(ratatui::widgets::Wrap { trim: true });
    }
    let header = format!(
        "{:<12}{:>9}  {:<14}{:>16}{:>14}{:>14}",
        "Accuracy", "Lessons", "Start date", "Average speed", "Top speed", "Characters"
    );
    let mut lines = vec![Line::from(Span::styled(header, s.name))];
    for streak in streaks {
        let n = streak.results.len();
        let avg = streak.results.iter().map(|r| r.speed).sum::<f64>() / n as f64;
        let top = streak.results.iter().map(|r| r.speed).fold(0.0, f64::max);
        let chars: u64 = streak.results.iter().map(|r| r.length as u64).sum();
        let start = crate::result::local_date(streak.results[0].time_stamp);
        lines.push(Line::from(Span::styled(
            format!(
                "{:<12}{:>9}  {:<14}{:>16}{:>14}{:>14}",
                format!("≥ {:.0}%", streak.level * 100.0),
                n,
                start.format("%Y-%m-%d").to_string(),
                unit.format(avg),
                unit.format(top),
                thousands(chars as f64),
            ),
            s.value,
        )));
    }
    Paragraph::new(lines)
}
