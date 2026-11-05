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
