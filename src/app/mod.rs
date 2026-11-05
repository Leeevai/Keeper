//! The application: screens, key bindings and the state they share.

pub mod practice;
pub mod profile;
pub mod settings_view;
pub mod typing_test;

use std::time::Instant;

use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::result::{DailyGoal, TypingResult};
use crate::settings::Settings;
use crate::sound::Sounds;
use crate::storage::{Storage, TestResult};
use crate::theme::Theme;
use crate::ui::Styles;
use practice::Practice;
use profile::{Profile, ProfileAction};
use settings_view::{SettingsAction, SettingsView};

/// What a key press does to the text being typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputAction {
    Char(u32),
    LineBreak,
    ClearChar,
    ClearWord,
}

impl InputAction {
    /// Maps a key press to a typing action, if it is one.
    pub fn from_key(key: &KeyEvent) -> Option<InputAction> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            // Windows reports characters typed with AltGr as Ctrl+Alt.
            KeyCode::Char(c) if ctrl == alt => Some(InputAction::Char(u32::from(c))),
            // Ctrl+W, and Ctrl+Backspace which many terminals send as Ctrl+H.
            KeyCode::Char('w' | 'h') if ctrl => Some(InputAction::ClearWord),
            KeyCode::Backspace if ctrl || alt => Some(InputAction::ClearWord),
            KeyCode::Backspace => Some(InputAction::ClearChar),
            KeyCode::Enter => Some(InputAction::LineBreak),
            _ => None,
        }
    }
}

/// Whether Ctrl is held without Alt: AltGr, reported as Ctrl+Alt on
/// Windows, types characters and must not trigger shortcuts.
fn is_ctrl(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Practice,
    TypingTest,
    Profile,
    Settings,
    Help,
}

impl Screen {
    pub const ALL: [Screen; 5] = [
        Screen::Practice,
        Screen::TypingTest,
        Screen::Profile,
        Screen::Settings,
        Screen::Help,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Screen::Practice => "Practice",
            Screen::TypingTest => "Typing test",
            Screen::Profile => "Profile",
            Screen::Settings => "Settings",
            Screen::Help => "Help",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
}

/// What the screens need to render themselves.
pub struct RenderContext<'a> {
    pub styles: &'a Styles,
    pub settings: &'a Settings,
    pub daily_goal: DailyGoal,
    pub toast: Option<&'a str>,
    /// Milliseconds since the start.
    pub now: f64,
    /// Whether the text cursor should be shown.
    pub focused: bool,
    pub data_dir: std::path::PathBuf,
}

/// Shortens a path in the home directory to `~/...`.
pub fn short_path(path: &std::path::Path) -> String {
    match dirs::home_dir().and_then(|home| path.strip_prefix(home).ok().map(|p| p.to_path_buf())) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// Drops the last hints until the rest fits in the width.
fn fit_hints(hints: &str, width: u16) -> String {
    let mut parts: Vec<&str> = hints.split(" · ").collect();
    while parts.len() > 1
        && unicode_width::UnicodeWidthStr::width(parts.join(" · ").as_str()) > width as usize
    {
        parts.pop();
    }
    parts.join(" · ")
}

/// Whether the lessons must be regenerated after a settings change.
fn lesson_changed(a: &Settings, b: &Settings) -> bool {
    let strip = |s: &Settings| Settings {
        theme: String::new(),
        color_mode: Default::default(),
        speed_unit: Default::default(),
        view: Default::default(),
        caret_shape: Default::default(),
        whitespace_style: Default::default(),
        play_sounds: Default::default(),
        sound_volume: 0.0,
        sound_theme: Default::default(),
        keyboard_colors: true,
        keyboard_pointers: true,
        daily_goal: 0,
        test_source: Default::default(),
        test_language: String::new(),
        test_word_list_size: 0,
        test_book: String::new(),
        test_paragraph_index: 0,
        test_duration: Default::default(),
        ..s.clone()
    };
    strip(a) != strip(b)
}
