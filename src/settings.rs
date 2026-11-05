//! All user settings, with the keys, defaults and ranges of keybr.com.

use serde::{Deserialize, Serialize};

use crate::keyboard::{Geometry, Keyboard, Layout};
use crate::language::Language;
use crate::result::TextType;
use crate::textinput::TextInputSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LessonType {
    /// Pseudo-words from the letters unlocked so far.
    #[default]
    Guided,
    /// The most common words of the language.
    Wordlist,
    Books,
    Custom,
    Code,
    Numbers,
}

impl LessonType {
    pub const ALL: [LessonType; 6] = [
        LessonType::Guided,
        LessonType::Wordlist,
        LessonType::Books,
        LessonType::Custom,
        LessonType::Code,
        LessonType::Numbers,
    ];

    pub fn name(self) -> &'static str {
        match self {
            LessonType::Guided => "Guided lessons",
            LessonType::Wordlist => "Common words",
            LessonType::Books => "Books",
            LessonType::Custom => "Custom text",
            LessonType::Code => "Source code",
            LessonType::Numbers => "Numbers",
        }
    }

    pub fn text_type(self) -> TextType {
        match self {
            LessonType::Guided => TextType::Generated,
            LessonType::Wordlist | LessonType::Books | LessonType::Custom => TextType::Natural,
            LessonType::Code => TextType::Code,
            LessonType::Numbers => TextType::Numbers,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SpeedUnit {
    #[default]
    Wpm,
    Wps,
    Cpm,
    Cps,
}

impl SpeedUnit {
    pub const ALL: [SpeedUnit; 4] = [
        SpeedUnit::Wpm,
        SpeedUnit::Wps,
        SpeedUnit::Cpm,
        SpeedUnit::Cps,
    ];

    pub fn id(self) -> &'static str {
        match self {
            SpeedUnit::Wpm => "wpm",
            SpeedUnit::Wps => "wps",
            SpeedUnit::Cpm => "cpm",
            SpeedUnit::Cps => "cps",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            SpeedUnit::Wpm => "Words per minute",
            SpeedUnit::Wps => "Words per second",
            SpeedUnit::Cpm => "Characters per minute",
            SpeedUnit::Cps => "Characters per second",
        }
    }

    /// Converts characters per minute into this unit.
    pub fn measure(self, cpm: f64) -> f64 {
        match self {
            SpeedUnit::Wpm => cpm / 5.0,
            SpeedUnit::Wps => cpm / 300.0,
            SpeedUnit::Cpm => cpm,
            SpeedUnit::Cps => cpm / 60.0,
        }
    }

    /// Formats a speed given in characters per minute, e.g. `42.5wpm`.
    pub fn format(self, cpm: f64) -> String {
        format!("{}{}", self.format_value(cpm), self.id())
    }

    pub fn format_value(self, cpm: f64) -> String {
        let v = self.measure(cpm);
        match self {
            SpeedUnit::Wpm | SpeedUnit::Cpm => format!("{v:.1}"),
            SpeedUnit::Wps | SpeedUnit::Cps => format!("{v:.2}"),
        }
    }
}
