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

    pub fn append_line_break(&mut self, time_stamp: f64, time_to_type: f64) -> Feedback {
        self.append_char(time_stamp, SPACE, time_to_type)
    }

    pub fn append_char(&mut self, time_stamp: f64, cp: u32, time_to_type: f64) -> Feedback {
        if self.completed() {
            return Feedback::Failed;
        }
        let expected = self.at(self.pos()).cp;

        if expected != SPACE && cp == SPACE {
            if self.settings.space_skips_words
                && ((self.pos() > 0 && self.at(self.pos() - 1).cp != SPACE) || self.typo)
            {
                self.skip_word(time_stamp);
                return Feedback::Recovered;
            }
            if self.garbage.is_empty() && !self.typo {
                // A space between words is ignored.
                return Feedback::Succeeded;
            }
        }

        if (expected == cp || normalize_cp(expected) == cp)
            && (self.settings.forgive_errors || self.garbage.is_empty())
        {
            let typo = self.typo;
            let ch = self.at(self.pos());
            self.add_step(
                Step {
                    time_stamp,
                    cp,
                    time_to_type,
                    typo,
                },
                ch,
            );
            self.garbage.clear();
            self.typo = false;
            return if typo {
                Feedback::Recovered
            } else {
                Feedback::Succeeded
            };
        }

        self.typo = true;
        if (!self.settings.stop_on_error || self.settings.forgive_errors)
            && self.garbage.len() < GARBAGE_BUFFER_LENGTH
        {
            self.garbage.push((
                Step {
                    time_stamp,
                    cp,
                    time_to_type,
                    typo: false,
                },
                Char {
                    cp,
                    attr: Attr::Garbage,
                    cls: None,
                },
            ));
        }
        if self.settings.forgive_errors
            && (self.handle_replaced_character() || self.handle_skipped_character())
        {
            Feedback::Recovered
        } else {
            Feedback::Failed
        }
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

/// Typing statistics for a single code point.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    #[serde(rename = "codePoint")]
    pub cp: u32,
    pub hit_count: u32,
    pub miss_count: u32,
    /// Average milliseconds to type the character, zero if unknown.
    pub time_to_type: u32,
}

impl Sample {
    pub fn is_valid(&self) -> bool {
        // Faster than 1500 CPM or slower than 5 CPM is noise.
        self.time_to_type == 0 || (40..=12000).contains(&self.time_to_type)
    }
}

/// Per-character statistics of a lesson.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Histogram {
    samples: BTreeMap<u32, Sample>,
}

impl Histogram {
    pub fn new(samples: impl IntoIterator<Item = Sample>) -> Self {
        Self {
            samples: samples.into_iter().map(|s| (s.cp, s)).collect(),
        }
    }

    pub fn from_steps(steps: &[Step]) -> Self {
        #[derive(Default)]
        struct Acc {
            hit: u32,
            miss: u32,
            time: f64,
            count: u32,
        }
        let mut map: BTreeMap<u32, Acc> = BTreeMap::new();
        for step in steps {
            let acc = map.entry(step.cp).or_default();
            acc.hit += 1;
            if step.typo {
                acc.miss += 1;
            } else if step.time_to_type > 0.0 {
                acc.time += step.time_to_type;
                acc.count += 1;
            }
        }
        Self::new(
            map.into_iter()
                .map(|(cp, a)| Sample {
                    cp,
                    hit_count: a.hit,
                    miss_count: a.miss,
                    time_to_type: if a.time > 0.0 && a.count > 0 {
                        (a.time / a.count as f64).round() as u32
                    } else {
                        0
                    },
                })
                .filter(Sample::is_valid),
        )
    }

    /// The number of distinct characters.
    pub fn complexity(&self) -> usize {
        self.samples.len()
    }

    pub fn get(&self, cp: u32) -> Option<&Sample> {
        self.samples.get(&cp)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Sample> {
        self.samples.values()
    }

    pub fn validate(&self) -> bool {
        self.samples.len() >= 3 && self.samples.values().all(Sample::is_valid)
    }
}

/// The outcome of typing a text.
#[derive(Debug, Clone, PartialEq)]
pub struct Stats {
    /// Milliseconds from the first to the last key press.
    pub time: u32,
    /// Characters per minute.
    pub speed: f64,
    pub length: u32,
    pub errors: u32,
    pub accuracy: f64,
    pub histogram: Histogram,
}

pub fn compute_speed(length: usize, time: f64) -> f64 {
    if time > 0.0 {
        length as f64 / (time / 1000.0) * 60.0
    } else {
        0.0
    }
}

pub fn make_stats(steps: &[Step]) -> Stats {
    if steps.len() >= 2 {
        let started = steps[0].time_stamp;
        let ended = steps[steps.len() - 1].time_stamp;
        let length = steps.len();
        let time = (ended - started).round().max(0.0);
        let errors = steps.iter().filter(|s| s.typo).count();
        Stats {
            time: time as u32,
            speed: compute_speed(length, time),
            length: length as u32,
            errors: errors as u32,
            accuracy: (length - errors) as f64 / length as f64,
            // The first key press only starts the clock.
            histogram: Histogram::from_steps(&steps[1..]),
        }
    } else {
        Stats {
            time: 0,
            speed: 0.0,
            length: 0,
            errors: 0,
            accuracy: 0.0,
            histogram: Histogram::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(text: &str, settings: TextInputSettings) -> TextInput {
        TextInput::new(plain(text), settings)
    }

    /// Types every character of `keys` 100ms apart.
    fn type_keys(ti: &mut TextInput, keys: &str) -> Vec<Feedback> {
        let start = ti.steps().last().map_or(0.0, |s| s.time_stamp);
        keys.chars()
            .enumerate()
            .map(|(i, c)| match c {
                '\u{8}' => ti.clear_char(),
                _ => ti.append_char(start + (i as f64 + 1.0) * 100.0, u32::from(c), 100.0),
            })
            .collect()
    }

    fn attrs(ti: &TextInput) -> String {
        ti.output()
            .iter()
            .map(|c| match c.attr {
                Attr::Normal => 'n',
                Attr::Hit => 'h',
                Attr::Miss => 'm',
                Attr::Garbage => 'g',
                Attr::Cursor => 'c',
            })
            .collect()
    }

    const STOP: TextInputSettings = TextInputSettings {
        stop_on_error: true,
        forgive_errors: false,
        space_skips_words: false,
    };
    const NO_STOP: TextInputSettings = TextInputSettings {
        stop_on_error: false,
        forgive_errors: false,
        space_skips_words: false,
    };
    const FORGIVE: TextInputSettings = TextInputSettings {
        stop_on_error: true,
        forgive_errors: true,
        space_skips_words: false,
    };

    #[test]
    fn types_text_without_errors() {
        let mut ti = input("abc", STOP);
        assert_eq!(attrs(&ti), "cnn");
        assert_eq!(type_keys(&mut ti, "abc"), vec![Feedback::Succeeded; 3]);
        assert!(ti.completed());
        assert_eq!(attrs(&ti), "hhh");
        assert_eq!(
            ti.append_char(1000.0, u32::from('x'), 1.0),
            Feedback::Failed
        );
    }

    #[test]
    fn stop_on_error_waits_for_the_right_key() {
        let mut ti = input("abc", STOP);
        assert_eq!(
            type_keys(&mut ti, "ax"),
            vec![Feedback::Succeeded, Feedback::Failed]
        );
        assert_eq!(ti.pos(), 1);
        assert_eq!(attrs(&ti), "hcn");
        assert_eq!(
            type_keys(&mut ti, "bc"),
            vec![Feedback::Recovered, Feedback::Succeeded]
        );
        assert_eq!(attrs(&ti), "hmh");
        assert_eq!(make_stats(&ti.steps()).errors, 1);
    }

    #[test]
    fn without_stop_garbage_must_be_deleted() {
        let mut ti = input("abc", NO_STOP);
        type_keys(&mut ti, "axy");
        assert_eq!(attrs(&ti), "hggcn");
        // The right key does not help while there is garbage.
        type_keys(&mut ti, "b");
        assert_eq!(attrs(&ti), "hgggcn");
        type_keys(&mut ti, "\u{8}\u{8}\u{8}b");
        assert_eq!(attrs(&ti), "hmc");
        assert_eq!(ti.pos(), 2);
    }

    #[test]
    fn forgives_a_replaced_character() {
        let mut ti = input("abcdef", FORGIVE);
        type_keys(&mut ti, "xbcd");
        assert_eq!(ti.pos(), 4);
        assert_eq!(attrs(&ti), "mhhhcn");
    }

    #[test]
    fn forgives_a_skipped_character() {
        let mut ti = input("abcdef", FORGIVE);
        type_keys(&mut ti, "bcd");
        assert_eq!(ti.pos(), 4);
        assert_eq!(attrs(&ti), "mhhhcn");
    }

    #[test]
    fn extra_space_between_words_is_ignored() {
        let mut ti = input("ab cd", STOP);
        type_keys(&mut ti, " ab  c");
        assert_eq!(ti.pos(), 4);
        assert_eq!(make_stats(&ti.steps()).errors, 0);
    }

    #[test]
    fn space_skips_words() {
        let settings = TextInputSettings {
            space_skips_words: true,
            ..STOP
        };
        let mut ti = input("abc def", settings);
        type_keys(&mut ti, "a ");
        assert_eq!(ti.pos(), 4);
        assert_eq!(attrs(&ti), "hmmhcnn");
    }

    #[test]
    fn typographic_characters_accept_plain_keys() {
        let mut ti = input("“a”", STOP);
        type_keys(&mut ti, "\"a\"");
        assert!(ti.completed());
        assert_eq!(make_stats(&ti.steps()).errors, 0);
    }

    #[test]
    fn clear_word_removes_the_current_word() {
        let mut ti = input("ab cd", STOP);
        type_keys(&mut ti, "ab c");
        ti.clear_word();
        assert_eq!(ti.pos(), 3);
        assert!(ti.has_typo());
    }

    #[test]
    fn stats_and_histogram() {
        let mut ti = input("abab", STOP);
        type_keys(&mut ti, "abxab");
        let stats = make_stats(&ti.steps());
        assert_eq!(stats.length, 4);
        assert_eq!(stats.errors, 1);
        assert_eq!(stats.time, 400);
        assert!((stats.speed - 600.0).abs() < 1e-9);
        assert!((stats.accuracy - 0.75).abs() < 1e-9);
        // The first step is excluded from the histogram.
        let a = stats.histogram.get(u32::from('a')).unwrap();
        assert_eq!((a.hit_count, a.miss_count, a.time_to_type), (1, 1, 0));
        let b = stats.histogram.get(u32::from('b')).unwrap();
        assert_eq!((b.hit_count, b.miss_count, b.time_to_type), (2, 0, 100));
        assert!(!stats.histogram.validate());
        assert_eq!(make_stats(&[]).length, 0);
    }

    #[test]
    fn histogram_drops_implausible_samples() {
        let steps: Vec<Step> = (0..4)
            .map(|i| Step {
                time_stamp: i as f64,
                cp: u32::from('a') + i,
                time_to_type: 10.0,
                typo: false,
            })
            .collect();
        assert_eq!(Histogram::from_steps(&steps).complexity(), 0);
    }
}
