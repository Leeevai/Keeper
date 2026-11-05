//! The application: screens, key bindings and the state they share.

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
