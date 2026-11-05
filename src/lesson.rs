//! Lessons, ported from `@keybr/lesson`.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

use crate::code::Syntax;
use crate::content::{Book, filter_word_list, word_list};
use crate::keyboard::{CodePointSet, Keyboard, Layout};
use crate::language::Language;
use crate::phonetic::{Filter, Letter, PhoneticModel};
use crate::result::{KeySample, KeyStatsMap, TypingResult, speed_to_time, time_to_speed};
use crate::rng::{Rng, random_sample, weighted_random_sample};
use crate::settings::{LessonType, Settings};
use crate::textinput::{StyledText, plain};
use crate::unicode::filter_text;

/// Loads a phonetic model once per language.
pub fn shared_model(language: &'static Language) -> Arc<PhoneticModel> {
    static CACHE: OnceLock<Mutex<HashMap<String, Arc<PhoneticModel>>>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    cache
        .entry(language.id.clone())
        .or_insert_with(|| {
            let model = PhoneticModel::load(language)
                .or_else(|_| PhoneticModel::load(Language::get("en")))
                .expect("the English model is embedded");
            Arc::new(model)
        })
        .clone()
}

/// The target typing speed which defines the confidence of a key.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    /// Characters per minute.
    pub target_speed: f64,
}

impl Target {
    pub fn new(settings: &Settings) -> Self {
        Self {
            target_speed: settings.target_speed as f64,
        }
    }

    /// The ratio of the target time to type to the actual one: 1.0 means
    /// the key is typed at the target speed.
    pub fn confidence(&self, time_to_type: Option<f64>) -> Option<f64> {
        time_to_type
            .filter(|t| t.is_finite() && *t > 0.0)
            .map(|t| speed_to_time(self.target_speed) / t)
    }
}

/// A key of a lesson with its learning progress.
#[derive(Debug, Clone, PartialEq)]
pub struct LessonKey {
    pub letter: Letter,
    pub samples: Vec<KeySample>,
    pub time_to_type: Option<f64>,
    pub best_time_to_type: Option<f64>,
    pub confidence: Option<f64>,
    pub best_confidence: Option<f64>,
    pub is_included: bool,
    pub is_focused: bool,
    /// Included manually by the alphabet size setting.
    pub is_forced: bool,
}

impl LessonKey {
    fn new(letter: &Letter, stats: &KeyStatsMap, target: &Target) -> Self {
        let (samples, time_to_type, best_time_to_type) = match stats.get(letter.cp) {
            Some(s) => (s.samples.clone(), s.time_to_type, s.best_time_to_type),
            None => (Vec::new(), None, None),
        };
        Self {
            letter: letter.clone(),
            samples,
            time_to_type,
            best_time_to_type,
            confidence: target.confidence(time_to_type),
            best_confidence: target.confidence(best_time_to_type),
            is_included: false,
            is_focused: false,
            is_forced: false,
        }
    }
}

/// The keys of a lesson, in the order they are unlocked.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LessonKeys {
    keys: Vec<LessonKey>,
}

impl LessonKeys {
    fn include_all(letters: &[Letter], stats: &KeyStatsMap, target: &Target) -> Self {
        Self {
            keys: letters
                .iter()
                .map(|l| LessonKey {
                    is_included: true,
                    ..LessonKey::new(l, stats, target)
                })
                .collect(),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &LessonKey> {
        self.keys.iter()
    }

    pub fn included(&self) -> impl Iterator<Item = &LessonKey> {
        self.keys.iter().filter(|k| k.is_included)
    }

    pub fn excluded(&self) -> impl Iterator<Item = &LessonKey> {
        self.keys.iter().filter(|k| !k.is_included)
    }

    pub fn focused(&self) -> Option<&LessonKey> {
        self.keys.iter().find(|k| k.is_focused)
    }

    pub fn find(&self, cp: u32) -> Option<&LessonKey> {
        self.keys.iter().find(|k| k.letter.cp == cp)
    }

    fn key_mut(&mut self, cp: u32) -> &mut LessonKey {
        self.keys
            .iter_mut()
            .find(|k| k.letter.cp == cp)
            .expect("lesson key")
    }

    fn include(&mut self, cp: u32) {
        self.key_mut(cp).is_included = true;
    }

    fn force(&mut self, cp: u32) {
        let key = self.key_mut(cp);
        key.is_included = true;
        key.is_forced = true;
    }

    fn focus(&mut self, cp: u32) {
        let key = self.key_mut(cp);
        key.is_included = true;
        key.is_focused = true;
    }
}

/// Dictionary words indexed by their letters.
struct Dictionary {
    words: Vec<String>,
    by_letter: HashMap<u32, Vec<usize>>,
}

impl Dictionary {
    fn new(words: Vec<String>) -> Self {
        let mut by_letter: HashMap<u32, Vec<usize>> = HashMap::new();
        for (i, word) in words.iter().enumerate() {
            let mut seen = HashSet::new();
            for c in word.chars() {
                if seen.insert(c) {
                    by_letter.entry(u32::from(c)).or_default().push(i);
                }
            }
        }
        Self { words, by_letter }
    }

    /// Finds the words which contain the focused letter and only allowed letters.
    fn find(&self, filter: &Filter) -> Vec<String> {
        let candidates: Box<dyn Iterator<Item = &String>> = match filter.focused {
            Some(cp) => Box::new(
                self.by_letter
                    .get(&cp)
                    .into_iter()
                    .flatten()
                    .map(|&i| &self.words[i]),
            ),
            None => Box::new(self.words.iter()),
        };
        candidates
            .filter(|w| w.chars().all(|c| filter.includes(u32::from(c))))
            .cloned()
            .collect()
    }
}

/// A stream of words.
trait WordSource {
    fn next(&mut self, rng: &mut dyn Rng) -> Option<String>;
}

struct PhoneticWords<'a> {
    model: &'a PhoneticModel,
    filter: &'a Filter,
}

impl WordSource for PhoneticWords<'_> {
    fn next(&mut self, rng: &mut dyn Rng) -> Option<String> {
        Some(self.model.next_word(self.filter, rng)).filter(|w| !w.is_empty())
    }
}

struct RandomWords<'a> {
    words: &'a [String],
}

impl WordSource for RandomWords<'_> {
    fn next(&mut self, rng: &mut dyn Rng) -> Option<String> {
        (!self.words.is_empty()).then(|| random_sample(self.words, rng).clone())
    }
}

struct WordSequence<'a> {
    words: &'a [String],
    index: &'a mut usize,
}

impl WordSource for WordSequence<'_> {
    fn next(&mut self, _: &mut dyn Rng) -> Option<String> {
        if self.words.is_empty() {
            return None;
        }
        if *self.index >= self.words.len() {
            *self.index = 0;
        }
        let word = self.words[*self.index].clone();
        *self.index += 1;
        Some(word)
    }
}

/// Avoids the same word twice in a row.
struct UniqueWords<S> {
    inner: S,
    last: String,
}

impl<S: WordSource> WordSource for UniqueWords<S> {
    fn next(&mut self, rng: &mut dyn Rng) -> Option<String> {
        let mut word = None;
        for _ in 0..3 {
            let w = self.inner.next(rng).filter(|w| !w.is_empty())?;
            if w != self.last {
                self.last = w.clone();
                return Some(w);
            }
            word = Some(w);
        }
        word
    }
}

/// Adds capital letters and punctuation to words.
struct MangledWords<'a, S> {
    inner: S,
    language: &'a Language,
    punctuators: Vec<Letter>,
    capitals: f64,
    punctuation: f64,
}

impl<S: WordSource> WordSource for MangledWords<'_, S> {
    fn next(&mut self, rng: &mut dyn Rng) -> Option<String> {
        let mut word = self.inner.next(rng).filter(|w| !w.is_empty())?;
        if self.capitals > 0.0 && self.capitals >= rng.next_f64() {
            word = self.language.capital_case(&word);
        }
        if self.punctuation > 0.0
            && !self.punctuators.is_empty()
            && self.punctuation >= rng.next_f64()
        {
            let p = weighted_random_sample(&self.punctuators, |l| l.f, rng).cp;
            word = match char::from_u32(p) {
                Some('!') => format!("{word}!"),
                Some('"') => format!("\"{word}\""),
                Some('\'') => format!("'{word}'"),
                Some(',') => format!("{word},"),
                Some('-') => match self.inner.next(rng) {
                    Some(next) => format!("{word}-{next}"),
                    None => word,
                },
                Some('.') => format!("{word}."),
                Some(':') => format!("{word}:"),
                Some(';') => format!("{word};"),
                Some('?') => format!("{word}?"),
                _ => word,
            };
        }
        Some(word)
    }
}

/// Joins words until the lesson is long enough, repeating each word.
fn generate_fragment(
    settings: &Settings,
    source: &mut dyn WordSource,
    repeat_words: u32,
    rng: &mut dyn Rng,
) -> String {
    let length = 100 + (settings.lesson_length * 100.0).round() as usize;
    let mut words: Vec<String> = Vec::new();
    let mut words_length = 0;
    loop {
        let word = source
            .next(rng)
            .filter(|w| !w.is_empty())
            .unwrap_or_else(|| "?".into());
        for _ in 0..repeat_words.max(1) {
            words_length += word.chars().count();
            words.push(word.clone());
            if words_length >= length {
                return words.join(" ");
            }
        }
    }
}

enum Kind {
    Guided {
        dictionary: Dictionary,
    },
    WordList {
        words: Vec<String>,
    },
    Sequence {
        words: Vec<String>,
        index: usize,
        randomize: bool,
    },
    Numbers,
    Code {
        syntax: &'static Syntax,
        flags: HashSet<String>,
    },
}

fn mangle_and_join(
    settings: &Settings,
    language: &Language,
    punctuators: Vec<Letter>,
    source: impl WordSource,
    rng: &mut dyn Rng,
) -> String {
    let mut mangled = MangledWords {
        inner: source,
        language,
        punctuators,
        capitals: settings.capitals,
        punctuation: settings.punctuators,
    };
    generate_fragment(settings, &mut mangled, settings.repeat_words, rng)
}

/// Removes characters which cannot be typed, optionally keeping letters only.
fn clean_text(
    text: &str,
    language: &Language,
    code_points: &CodePointSet,
    letters_only: bool,
    lowercase: bool,
) -> String {
    let filtered = filter_text(text, |cp| {
        code_points.has(cp) && (!letters_only || language.includes(cp))
    });
    if lowercase {
        language.lower_case(&filtered)
    } else {
        filtered
    }
}

fn generate_numbers(benford: bool, rng: &mut dyn Rng) -> String {
    let digits = Letter::digits();
    let non_zero = &digits[1..];
    let mut words = Vec::new();
    let mut words_length = 0;
    while words_length < 50 {
        let length = (3.0 + rng.next_f64() * 4.0).floor() as usize;
        let mut word = String::new();
        let mut last = None;
        for i in 0..length {
            loop {
                let digit = if i == 0 {
                    if benford {
                        weighted_random_sample(non_zero, |l| l.f, rng)
                    } else {
                        random_sample(non_zero, rng)
                    }
                } else {
                    random_sample(&digits, rng)
                };
                if Some(digit.cp) != last {
                    word.push(char::from_u32(digit.cp).unwrap_or('0'));
                    last = Some(digit.cp);
                    break;
                }
            }
        }
        words_length += word.len();
        words.push(word);
    }
    words.join(" ")
}
