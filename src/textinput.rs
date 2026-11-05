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
