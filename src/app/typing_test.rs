//! The typing test screen, ported from keybr's typing test page.

use std::sync::Arc;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Sparkline};

use super::{InputAction, RenderContext};
use crate::content::{Book, word_list};
use crate::language::Language;
use crate::lesson::shared_model;
use crate::phonetic::{Filter, PhoneticModel};
use crate::rng::{Lcg, random_sample};
use crate::settings::{Settings, TestDuration, TestSource};
use crate::stats::Distribution;
use crate::storage::TestResult;
use crate::textinput::{Char, Feedback, Step, TextInput, compute_speed, make_stats, plain};
use crate::ui::text::TextView;
use crate::ui::{format_duration, thousands};
use unicode_width::UnicodeWidthStr;

const NUM_LINES: usize = 5;
const NUM_COLS: usize = 55;

enum Source {
    Words(Vec<String>),
    Pseudo(Arc<PhoneticModel>),
    Book { words: Vec<String>, index: usize },
}

impl Source {
    fn new(settings: &Settings) -> Self {
        match settings.test_source {
            TestSource::CommonWords => {
                let words: Vec<String> = word_list(&settings.test_language)
                    .iter()
                    .take(settings.test_word_list_size as usize)
                    .cloned()
                    .collect();
                if words.is_empty() {
                    Source::Words(vec!["?".into()])
                } else {
                    Source::Words(words)
                }
            }
            TestSource::PseudoWords => {
                Source::Pseudo(shared_model(Language::get(&settings.test_language)))
            }
            TestSource::Book => {
                let paragraphs = Book::get(&settings.test_book).paragraphs();
                let start = (settings.test_paragraph_index as usize).min(paragraphs.len());
                let words = paragraphs[start..]
                    .iter()
                    .chain(&paragraphs[..start])
                    .flat_map(|p| p.split_whitespace().map(String::from).collect::<Vec<_>>())
                    .collect();
                Source::Book { words, index: 0 }
            }
        }
    }

    fn next_word(&mut self, rng: &mut Lcg) -> String {
        match self {
            Source::Words(words) => random_sample(words, rng).clone(),
            Source::Pseudo(model) => {
                let w = model.next_word(&Filter::default(), rng);
                if w.is_empty() { "?".into() } else { w }
            }
            Source::Book { words, index } => {
                if words.is_empty() {
                    return "?".into();
                }
                if *index >= words.len() {
                    *index = 0;
                }
                *index += 1;
                words[*index - 1].clone()
            }
        }
    }
}

/// The outcome of a finished test.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub result: TestResult,
    /// Speed over the test, in characters per minute per two seconds.
    pub speeds: Vec<f64>,
}

pub struct TypingTest {
    source: Source,
    rng: Lcg,
    duration: TestDuration,
    source_id: String,
    lines: Vec<String>,
    /// What was typed on the lines above the active one.
    typed: Vec<Option<Vec<Char>>>,
    active: usize,
    input: TextInput,
    steps: Vec<Step>,
    last_input: f64,
    pub report: Option<Report>,
}

impl TypingTest {
    pub fn new(settings: &Settings) -> Self {
        Self::with_rng(settings, Lcg::from_time())
    }

    pub fn with_rng(settings: &Settings, mut rng: Lcg) -> Self {
        let mut source = Source::new(settings);
        let lines: Vec<String> = (0..NUM_LINES)
            .map(|_| generate_line(&mut source, &mut rng))
            .collect();
        let input = TextInput::new(plain(lines[0].clone()), settings.text_input());
        Self {
            source,
            rng,
            duration: settings.test_duration,
            source_id: id_of(settings.test_source),
            typed: vec![None; NUM_LINES],
            lines,
            active: 0,
            input,
            steps: Vec::new(),
            last_input: 0.0,
            report: None,
        }
    }

    /// The next character to type.
    pub fn next_char(&self) -> Option<char> {
        self.input
            .remaining()
            .first()
            .and_then(|&cp| char::from_u32(cp))
    }
}

/// The serialized name of a setting value, e.g. `seconds15`.
fn id_of<T: serde::Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

fn plain_chars(line: &str) -> Vec<Char> {
    line.chars()
        .map(|c| Char {
            cp: u32::from(c),
            attr: crate::textinput::Attr::Normal,
            cls: None,
        })
        .collect()
}

/// A line of words of at most `NUM_COLS` characters, ending with a space.
fn generate_line(source: &mut Source, rng: &mut Lcg) -> String {
    let mut line = String::new();
    let mut words = 0;
    loop {
        let state = (
            rng.clone(),
            match source {
                Source::Book { index, .. } => *index,
                _ => 0,
            },
        );
        let word = source.next_word(rng);
        // Measured in terminal columns: some scripts are two columns wide.
        if words > 0 && line.width() + word.width() + 1 > NUM_COLS {
            // Put the word back for the next line.
            *rng = state.0;
            if let Source::Book { index, .. } = source {
                *index = state.1;
            }
            break;
        }
        line.push_str(&word);
        line.push(' ');
        words += 1;
    }
    line
}

/// Typing speed in two second windows.
fn speed_series(steps: &[Step]) -> Vec<f64> {
    let Some(first) = steps.first() else {
        return Vec::new();
    };
    let window = 2000.0;
    let end = steps.last().map_or(0.0, |s| s.time_stamp);
    let buckets = (((end - first.time_stamp) / window).ceil() as usize).max(1);
    let mut counts = vec![0usize; buckets];
    for step in steps {
        let i = (((step.time_stamp - first.time_stamp) / window) as usize).min(buckets - 1);
        counts[i] += 1;
    }
    counts
        .into_iter()
        .map(|n| n as f64 / (window / 1000.0) * 60.0)
        .collect()
}
