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

/// Generates the text of lessons and tracks which keys are being learned.
pub struct Lesson {
    pub lesson_type: LessonType,
    pub settings: Settings,
    pub keyboard: Keyboard,
    pub code_points: CodePointSet,
    pub language: &'static Language,
    model: Arc<PhoneticModel>,
    letters: Vec<Letter>,
    kind: Kind,
}

impl Lesson {
    pub fn new(settings: &Settings) -> Self {
        let options = settings.keyboard_options();
        let keyboard = settings.keyboard();
        let code_points = keyboard.code_points();
        let language = options.language;
        let model = shared_model(language);
        let model_letters: Vec<Letter> = model
            .letters
            .iter()
            .filter(|l| code_points.has(l.cp))
            .cloned()
            .collect();
        let allowed = |cp: u32| code_points.has(cp);
        let (letters, kind) = match settings.lesson_type {
            LessonType::Guided => {
                let words: Vec<String> = filter_word_list(word_list(&language.id), allowed)
                    .into_iter()
                    .filter(|w| w.chars().count() > 2)
                    .collect();
                (
                    model_letters,
                    Kind::Guided {
                        dictionary: Dictionary::new(words),
                    },
                )
            }
            LessonType::Wordlist => {
                let words = filter_word_list(word_list(&language.id), allowed)
                    .into_iter()
                    .filter(|w| !settings.long_words_only || w.chars().count() > 3)
                    .take(settings.word_list_size as usize)
                    .collect();
                (model_letters, Kind::WordList { words })
            }
            LessonType::Books => {
                let book = Book::get(&settings.book);
                let paragraphs: Vec<String> = book
                    .paragraphs()
                    .iter()
                    .map(|p| {
                        clean_text(
                            p,
                            language,
                            &code_points,
                            settings.book_letters_only,
                            settings.book_lowercase,
                        )
                    })
                    .collect();
                let start = (settings.paragraph_index as usize).min(paragraphs.len());
                let words = paragraphs[start..]
                    .iter()
                    .chain(&paragraphs[..start])
                    .flat_map(|p| p.split_whitespace().map(String::from).collect::<Vec<_>>())
                    .collect();
                (
                    model_letters,
                    Kind::Sequence {
                        words,
                        index: 0,
                        randomize: false,
                    },
                )
            }
            LessonType::Custom => {
                let text = clean_text(
                    &settings.custom_text,
                    language,
                    &code_points,
                    settings.custom_letters_only,
                    settings.custom_lowercase,
                );
                let words = text.split_whitespace().map(String::from).collect();
                (
                    model_letters,
                    Kind::Sequence {
                        words,
                        index: 0,
                        randomize: settings.custom_randomize,
                    },
                )
            }
            LessonType::Numbers => (Letter::digits(), Kind::Numbers),
            LessonType::Code => (
                Letter::programming(),
                Kind::Code {
                    syntax: Syntax::get(&settings.code_syntax),
                    flags: settings.code_flags.iter().cloned().collect(),
                },
            ),
        };
        Self {
            lesson_type: settings.lesson_type,
            settings: settings.clone(),
            keyboard,
            code_points,
            language,
            model,
            letters,
            kind,
        }
    }

    /// The letters whose statistics are tracked by this lesson.
    pub fn letters(&self) -> &[Letter] {
        &self.letters
    }

    pub fn layout(&self) -> &'static Layout {
        self.keyboard.layout
    }

    /// Keeps the results typed on a layout of the same family.
    pub fn filter<'a>(&self, results: &'a [TypingResult]) -> Vec<&'a TypingResult> {
        let family = &self.keyboard.layout.family;
        results
            .iter()
            .filter(|r| Layout::find(&r.layout).is_some_and(|l| &l.family == family))
            .collect()
    }

    pub fn key_stats(&self, results: &[TypingResult]) -> KeyStatsMap {
        KeyStatsMap::from_results(&self.letters, self.filter(results))
    }

    /// Decides which keys are included and which key is focused.
    pub fn update(&self, stats: &KeyStatsMap) -> LessonKeys {
        let target = Target::new(&self.settings);
        match self.kind {
            Kind::Guided { .. } => self.update_guided(stats, &target),
            _ => LessonKeys::include_all(&self.letters, stats, &target),
        }
    }

    fn ordered_letters(&self) -> Vec<Letter> {
        if self.settings.keyboard_order {
            Letter::weighted_frequency_order(&self.letters, |l| self.code_points.weight(l.cp))
        } else {
            Letter::frequency_order(&self.letters)
        }
    }

    fn update_guided(&self, stats: &KeyStatsMap, target: &Target) -> LessonKeys {
        let recover_keys = self.settings.recover_keys;
        let letters = self.ordered_letters();
        let min_size = 6usize;
        let max_size = min_size
            + ((letters.len().saturating_sub(min_size)) as f64 * self.settings.alphabet_size)
                .round() as usize;
        let mut keys = LessonKeys {
            keys: letters
                .iter()
                .map(|l| LessonKey::new(l, stats, target))
                .collect(),
        };
        for i in 0..keys.keys.len() {
            let key = keys.keys[i].clone();
            let included: Vec<&LessonKey> = keys.included().collect();
            if included.len() < min_size {
                keys.include(key.letter.cp);
                continue;
            }
            if included.len() < max_size {
                keys.force(key.letter.cp);
                continue;
            }
            if key.best_confidence.unwrap_or(0.0) >= 1.0 {
                keys.include(key.letter.cp);
                continue;
            }
            let all_confident = if recover_keys {
                included.iter().all(|k| k.confidence.unwrap_or(0.0) >= 1.0)
            } else {
                included
                    .iter()
                    .all(|k| k.best_confidence.unwrap_or(0.0) >= 1.0)
            };
            if all_confident {
                keys.include(key.letter.cp);
            }
        }
        // Focus on the least confident of the included keys.
        let confidence_of = |k: &LessonKey| {
            if recover_keys {
                k.confidence.unwrap_or(0.0)
            } else {
                k.best_confidence.unwrap_or(0.0)
            }
        };
        let mut weakest: Vec<&LessonKey> =
            keys.included().filter(|k| confidence_of(k) < 1.0).collect();
        weakest.sort_by(|a, b| confidence_of(a).total_cmp(&confidence_of(b)));
        if let Some(cp) = weakest.first().map(|k| k.letter.cp) {
            keys.focus(cp);
        }
        keys
    }

    /// Generates the text of the next lesson.
    pub fn generate(&mut self, keys: &LessonKeys, rng: &mut dyn Rng) -> StyledText {
        let settings = &self.settings;
        let punctuators: Vec<Letter> = Letter::punctuators()
            .into_iter()
            .filter(|l| self.code_points.has(l.cp))
            .collect();
        match &mut self.kind {
            Kind::Guided { dictionary } => {
                let included: Vec<u32> = keys.included().map(|k| k.letter.cp).collect();
                let filter = if included.is_empty() {
                    Filter::default()
                } else {
                    Filter::new(&included, keys.focused().map(|k| k.letter.cp))
                };
                let mut pseudo = PhoneticWords {
                    model: &self.model,
                    filter: &filter,
                };
                let natural = settings.natural_words.then(|| {
                    let mut words: Vec<String> =
                        dictionary.find(&filter).into_iter().take(1000).collect();
                    while words.len() < 15 {
                        match pseudo.next(rng) {
                            Some(w) => words.push(w),
                            None => break,
                        }
                    }
                    if words.is_empty() {
                        words.push("?".into());
                    }
                    words
                });
                let text = match &natural {
                    Some(words) => {
                        let source = UniqueWords {
                            inner: RandomWords { words },
                            last: String::new(),
                        };
                        mangle_and_join(settings, self.language, punctuators, source, rng)
                    }
                    None => {
                        let source = UniqueWords {
                            inner: pseudo,
                            last: String::new(),
                        };
                        mangle_and_join(settings, self.language, punctuators, source, rng)
                    }
                };
                plain(text)
            }
            Kind::WordList { words } => {
                let source = UniqueWords {
                    inner: RandomWords { words },
                    last: String::new(),
                };
                plain(mangle_and_join(
                    settings,
                    self.language,
                    punctuators,
                    source,
                    rng,
                ))
            }
            Kind::Sequence {
                words,
                index,
                randomize,
            } => {
                let text = if *randomize && !words.is_empty() {
                    let mut source = UniqueWords {
                        inner: RandomWords { words },
                        last: String::new(),
                    };
                    generate_fragment(settings, &mut source, 1, rng)
                } else {
                    let mut source = WordSequence { words, index };
                    generate_fragment(settings, &mut source, 1, rng)
                };
                plain(text)
            }
            Kind::Numbers => plain(generate_numbers(settings.benford, rng)),
            Kind::Code { syntax, flags } => syntax.generate(flags, rng),
        }
    }
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

/// The latest run of lessons without long breaks or slowdowns.
fn find_session(samples: &[KeySample]) -> &[KeySample] {
    let n = samples.len();
    for i in (1..n).rev() {
        let a = &samples[i - 1];
        let b = &samples[i];
        if b.time_stamp - a.time_stamp > 3_600_000 {
            return &samples[i..];
        }
        if b.filtered_time_to_type > a.filtered_time_to_type && n - i + 1 >= 5 {
            return &samples[i..];
        }
    }
    samples
}

fn eval(coefficients: &[f64], x: f64) -> f64 {
    coefficients.iter().rev().fold(0.0, |acc, c| acc * x + c)
}

fn derivative(coefficients: &[f64]) -> Vec<f64> {
    coefficients
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, c)| c * i as f64)
        .collect()
}

/// Least squares fit of a polynomial, coefficients from the constant term up.
pub fn polynomial_regression(xs: &[f64], ys: &[f64], degree: usize) -> Option<Vec<f64>> {
    let size = degree + 1;
    // The normal equations, augmented with the right hand side.
    let mut a: Vec<Vec<f64>> = (0..size)
        .map(|i| {
            let mut row: Vec<f64> = (0..size)
                .map(|j| xs.iter().map(|x| x.powi((i + j) as i32)).sum())
                .collect();
            row.push(xs.iter().zip(ys).map(|(x, y)| x.powi(i as i32) * y).sum());
            row
        })
        .collect();
    // Gaussian elimination with partial pivoting.
    for col in 0..size {
        let pivot = (col..size).max_by(|&p, &q| a[p][col].abs().total_cmp(&a[q][col].abs()))?;
        if a[pivot][col].abs() < 1e-12 {
            return None;
        }
        a.swap(col, pivot);
        let (top, bottom) = a.split_at_mut(col + 1);
        let pivot_row = &top[col];
        for row in bottom.iter_mut() {
            let f = row[col] / pivot_row[col];
            for (v, p) in row.iter_mut().zip(pivot_row).skip(col) {
                *v -= f * p;
            }
        }
    }
    let mut x = vec![0.0; size];
    for row in (0..size).rev() {
        let s: f64 = (row + 1..size).map(|k| a[row][k] * x[k]).sum();
        x[row] = (a[row][size] - s) / a[row][row];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}
