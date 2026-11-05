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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CaretShape {
    Block,
    Box,
    Line,
    #[default]
    Underline,
}

impl CaretShape {
    pub const ALL: [CaretShape; 4] = [
        CaretShape::Block,
        CaretShape::Box,
        CaretShape::Line,
        CaretShape::Underline,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CaretShape::Block => "Block cursor",
            CaretShape::Box => "Box cursor",
            CaretShape::Line => "Line cursor",
            CaretShape::Underline => "Underline cursor",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum WhitespaceStyle {
    Space,
    Bar,
    #[default]
    Bullet,
}

impl WhitespaceStyle {
    pub const ALL: [WhitespaceStyle; 3] = [
        WhitespaceStyle::Space,
        WhitespaceStyle::Bar,
        WhitespaceStyle::Bullet,
    ];

    pub fn name(self) -> &'static str {
        match self {
            WhitespaceStyle::Space => "No whitespace",
            WhitespaceStyle::Bar => "Bar whitespace",
            WhitespaceStyle::Bullet => "Bullet whitespace",
        }
    }

    pub fn symbol(self) -> char {
        match self {
            WhitespaceStyle::Space => ' ',
            WhitespaceStyle::Bar => '␣',
            WhitespaceStyle::Bullet => '·',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PlaySounds {
    #[default]
    None,
    ErrorsOnly,
    All,
    KeysOnly,
}

impl PlaySounds {
    pub const ALL: [PlaySounds; 4] = [
        PlaySounds::None,
        PlaySounds::ErrorsOnly,
        PlaySounds::All,
        PlaySounds::KeysOnly,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PlaySounds::None => "No sounds",
            PlaySounds::ErrorsOnly => "Error sounds only",
            PlaySounds::All => "All sounds",
            PlaySounds::KeysOnly => "Key sounds only",
        }
    }

    pub fn keys(self) -> bool {
        matches!(self, PlaySounds::All | PlaySounds::KeysOnly)
    }

    pub fn errors(self) -> bool {
        matches!(self, PlaySounds::All | PlaySounds::ErrorsOnly)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SoundTheme {
    #[default]
    Default,
    Mechanical1,
    Mechanical2,
    Typewriter1,
    Typewriter2,
}

impl SoundTheme {
    pub const ALL: [SoundTheme; 5] = [
        SoundTheme::Default,
        SoundTheme::Mechanical1,
        SoundTheme::Mechanical2,
        SoundTheme::Typewriter1,
        SoundTheme::Typewriter2,
    ];

    pub fn name(self) -> &'static str {
        match self {
            SoundTheme::Default => "Default",
            SoundTheme::Mechanical1 => "Mechanical 1",
            SoundTheme::Mechanical2 => "Mechanical 2",
            SoundTheme::Typewriter1 => "Typewriter 1",
            SoundTheme::Typewriter2 => "Typewriter 2",
        }
    }
}

/// How typed characters are mapped to the practiced layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Emulation {
    /// Characters are used as typed.
    None,
    /// Keys typed on the system layout are translated into the practiced layout.
    #[default]
    Forward,
    /// The keyboard hardware already emulates the layout.
    Reverse,
}

impl Emulation {
    pub const ALL: [Emulation; 3] = [Emulation::None, Emulation::Forward, Emulation::Reverse];

    pub fn name(self) -> &'static str {
        match self {
            Emulation::None => "Off",
            Emulation::Forward => "Emulate layout",
            Emulation::Reverse => "Keyboard hardware emulates layout",
        }
    }
}

/// How much of the practice screen is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum View {
    #[default]
    Normal,
    Compact,
    Bare,
}

impl View {
    pub fn next(self) -> View {
        match self {
            View::Normal => View::Compact,
            View::Compact => View::Bare,
            View::Bare => View::Normal,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            View::Normal => "Normal",
            View::Compact => "Compact",
            View::Bare => "Bare",
        }
    }
}

/// Terminal color support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode {
    #[default]
    Auto,
    Truecolor,
    #[serde(rename = "256")]
    Ansi256,
}

impl ColorMode {
    pub const ALL: [ColorMode; 3] = [ColorMode::Auto, ColorMode::Truecolor, ColorMode::Ansi256];

    pub fn name(self) -> &'static str {
        match self {
            ColorMode::Auto => "Detect",
            ColorMode::Truecolor => "24-bit color",
            ColorMode::Ansi256 => "256 colors",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TestSource {
    #[default]
    CommonWords,
    PseudoWords,
    Book,
}

impl TestSource {
    pub const ALL: [TestSource; 3] = [
        TestSource::CommonWords,
        TestSource::PseudoWords,
        TestSource::Book,
    ];

    pub fn name(self) -> &'static str {
        match self {
            TestSource::CommonWords => "Common words",
            TestSource::PseudoWords => "Pseudo words",
            TestSource::Book => "Book",
        }
    }
}
