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
