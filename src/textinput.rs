//! The typing state machine, ported from `@keybr/textinput`.

use std::collections::BTreeMap;

use crate::unicode::{SPACE, normalize_cp};

/// How a character of the text is displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attr {
    /// Not typed yet.
    Normal,
    /// Typed correctly at the first attempt.
    Hit,
    /// Typed correctly after an error.
    Miss,
    /// A wrong character typed by mistake, displayed when the cursor
    /// does not stop on errors.
    Garbage,
    /// The character at the cursor.
    Cursor,
}

/// A syntax class of a piece of source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Keyword,
    String,
    Number,
    Comment,
    Other,
}

impl Class {
    pub fn parse(s: &str) -> Class {
        match s {
            "keyword" => Class::Keyword,
            "string" => Class::String,
            "number" => Class::Number,
            "comment" => Class::Comment,
            _ => Class::Other,
        }
    }
}

/// A span of text, optionally with a syntax class.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub text: String,
    pub cls: Option<Class>,
}

/// Text made of spans.
pub type StyledText = Vec<Span>;

pub fn plain(text: impl Into<String>) -> StyledText {
    vec![Span {
        text: text.into(),
        cls: None,
    }]
}

pub fn flatten(text: &StyledText) -> String {
    text.iter().map(|s| s.text.as_str()).collect()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Char {
    pub cp: u32,
    pub attr: Attr,
    pub cls: Option<Class>,
}

fn split(text: &StyledText) -> Vec<Char> {
    text.iter()
        .flat_map(|span| {
            span.text.chars().map(move |c| Char {
                cp: u32::from(c),
                attr: Attr::Normal,
                cls: span.cls,
            })
        })
        .collect()
}

/// A single typed character.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    /// Milliseconds since an arbitrary epoch.
    pub time_stamp: f64,
    pub cp: u32,
    /// Milliseconds since the previous key press.
    pub time_to_type: f64,
    /// Whether the character was typed after an error.
    pub typo: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feedback {
    Succeeded,
    Recovered,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextInputSettings {
    /// Stop the cursor until the right key is pressed.
    pub stop_on_error: bool,
    /// Recover automatically from a replaced or a skipped character.
    pub forgive_errors: bool,
    /// The space key skips the rest of the current word.
    pub space_skips_words: bool,
}

impl Default for TextInputSettings {
    fn default() -> Self {
        Self {
            stop_on_error: true,
            forgive_errors: true,
            space_skips_words: false,
        }
    }
}

const RECOVER_BUFFER_LENGTH: usize = 3;
const GARBAGE_BUFFER_LENGTH: usize = 10;

pub struct TextInput {
    pub text: StyledText,
    pub settings: TextInputSettings,
    chars: Vec<Char>,
    steps: Vec<(Step, Char)>,
    garbage: Vec<(Step, Char)>,
    typo: bool,
}

impl TextInput {
    pub fn new(text: StyledText, settings: TextInputSettings) -> Self {
        let chars = split(&text);
        Self {
            text,
            settings,
            chars,
            steps: Vec::new(),
            garbage: Vec::new(),
            typo: false,
        }
    }

    pub fn reset(&mut self) {
        self.steps.clear();
        self.garbage.clear();
        self.typo = false;
    }

    pub fn len(&self) -> usize {
        self.chars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    pub fn at(&self, index: usize) -> Char {
        self.chars[index]
    }

    /// The position of the cursor.
    pub fn pos(&self) -> usize {
        self.steps.len()
    }

    pub fn completed(&self) -> bool {
        self.pos() == self.len()
    }
}
