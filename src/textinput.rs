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

    pub fn steps(&self) -> Vec<Step> {
        self.steps.iter().map(|(s, _)| *s).collect()
    }

    pub fn has_typo(&self) -> bool {
        self.typo
    }

    /// The characters to display: typed, garbage, the cursor and the rest.
    pub fn output(&self) -> Vec<Char> {
        let mut out: Vec<Char> = self.steps.iter().map(|(_, c)| *c).collect();
        if !self.settings.stop_on_error {
            out.extend(self.garbage.iter().map(|(_, c)| *c));
        }
        let remaining = &self.chars[self.pos()..];
        if let Some((head, tail)) = remaining.split_first() {
            out.push(Char {
                attr: Attr::Cursor,
                ..*head
            });
            out.extend_from_slice(tail);
        }
        out
    }

    /// The code points still to be typed.
    pub fn remaining(&self) -> Vec<u32> {
        self.chars[self.pos()..].iter().map(|c| c.cp).collect()
    }

    pub fn clear_char(&mut self) -> Feedback {
        self.garbage.pop();
        self.typo = true;
        Feedback::Succeeded
    }

    pub fn clear_word(&mut self) -> Feedback {
        self.garbage.clear();
        while self.pos() > 0 && self.at(self.pos() - 1).cp != SPACE {
            self.steps.pop();
        }
        self.typo = true;
        Feedback::Succeeded
    }

    fn add_step(&mut self, step: Step, ch: Char) {
        let attr = if step.typo { Attr::Miss } else { Attr::Hit };
        self.steps.push((step, Char { attr, ..ch }));
    }

    fn skip_word(&mut self, time_stamp: f64) {
        while self.pos() < self.len() && self.at(self.pos()).cp != SPACE {
            let ch = self.at(self.pos());
            self.add_step(
                Step {
                    time_stamp,
                    cp: ch.cp,
                    time_to_type: 0.0,
                    typo: true,
                },
                ch,
            );
        }
        if self.pos() < self.len() && self.at(self.pos()).cp == SPACE {
            let ch = self.at(self.pos());
            self.add_step(
                Step {
                    time_stamp,
                    cp: ch.cp,
                    time_to_type: 0.0,
                    typo: false,
                },
                ch,
            );
        }
        self.garbage.clear();
        self.typo = false;
    }

    /// text: `abcd`, garbage: `xbcd`.
    fn handle_replaced_character(&mut self) -> bool {
        if self.pos() + RECOVER_BUFFER_LENGTH + 1 > self.len()
            || self.garbage.len() < RECOVER_BUFFER_LENGTH + 1
        {
            return false;
        }
        for i in 0..RECOVER_BUFFER_LENGTH {
            if self.at(self.pos() + i + 1).cp != self.garbage[i + 1].0.cp {
                return false;
            }
        }
        let garbage = std::mem::take(&mut self.garbage);
        let ch = self.at(self.pos());
        self.add_step(
            Step {
                time_stamp: garbage[0].0.time_stamp,
                cp: ch.cp,
                time_to_type: 0.0,
                typo: true,
            },
            ch,
        );
        for (step, _) in &garbage[1..] {
            let ch = self.at(self.pos());
            self.add_step(*step, ch);
        }
        self.typo = false;
        true
    }

    /// text: `abcd`, garbage: `bcd`.
    fn handle_skipped_character(&mut self) -> bool {
        if self.pos() + RECOVER_BUFFER_LENGTH + 1 > self.len()
            || self.garbage.len() < RECOVER_BUFFER_LENGTH
        {
            return false;
        }
        for i in 0..RECOVER_BUFFER_LENGTH {
            if self.at(self.pos() + i + 1).cp != self.garbage[i].0.cp {
                return false;
            }
        }
        let garbage = std::mem::take(&mut self.garbage);
        let ch = self.at(self.pos());
        self.add_step(
            Step {
                time_stamp: garbage[0].0.time_stamp,
                cp: ch.cp,
                time_to_type: 0.0,
                typo: true,
            },
            ch,
        );
        for (step, _) in &garbage {
            let ch = self.at(self.pos());
            self.add_step(*step, ch);
        }
        self.typo = false;
        true
    }
}
