//! The phonetic word generator, ported from `@keybr/phonetic-model`.
//!
//! A language model is a Markov chain over letters. Words are generated
//! one letter at a time, using only the letters allowed by a [`Filter`],
//! which makes it possible to produce pronounceable pseudo-words from
//! a small set of keys.

use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail, ensure};

use crate::assets;
use crate::language::Language;
use crate::rng::{Rng, weighted_random_sample};
use crate::unicode::{SPACE, from_code_points};

const SIGNATURE: &[u8] = b"keybr.com";
const MIN_LENGTH: usize = 3;
const MAX_LENGTH: usize = 10;

/// A letter with its relative frequency in the language.
#[derive(Debug, Clone, PartialEq)]
pub struct Letter {
    pub cp: u32,
    /// Relative frequency.
    pub f: f64,
    /// Display label, usually the uppercase variant.
    pub label: String,
}

impl Letter {
    pub fn new(cp: u32, f: f64) -> Self {
        Self {
            cp,
            f,
            label: crate::unicode::cp_to_string(cp),
        }
    }

    pub fn with_label(cp: u32, f: f64, label: String) -> Self {
        Self { cp, f, label }
    }

    pub fn digits() -> Vec<Letter> {
        [
            ('0', 0.0),
            ('1', 0.301),
            ('2', 0.176),
            ('3', 0.125),
            ('4', 0.097),
            ('5', 0.079),
            ('6', 0.067),
            ('7', 0.058),
            ('8', 0.051),
            ('9', 0.046),
        ]
        .into_iter()
        .map(|(c, f)| Letter::new(u32::from(c), f))
        .collect()
    }

    pub fn punctuators() -> Vec<Letter> {
        [
            (',', 9.0),
            ('.', 8.0),
            ('!', 2.0),
            ('?', 2.0),
            (';', 1.0),
            (':', 1.0),
            ('\'', 1.0),
            ('"', 1.0),
            ('-', 1.0),
        ]
        .into_iter()
        .map(|(c, f)| Letter::new(u32::from(c), f))
        .collect()
    }

    /// Special characters, other than the basic punctuation.
    pub fn specials() -> Vec<Letter> {
        "#$%&()*+-/<=>@[\\]^_`{|}~"
            .chars()
            .map(|c| Letter::new(u32::from(c), 1.0))
            .collect()
    }

    /// Characters of programming languages; paired characters have their
    /// frequencies halved as they are used in pairs.
    pub fn programming() -> Vec<Letter> {
        [
            ('(', 2.5),
            (')', 2.4),
            ('{', 1.8),
            ('}', 1.7),
            ('"', 1.7),
            ('\'', 1.7),
            (',', 1.6),
            (';', 1.5),
            ('[', 1.5),
            (']', 1.5),
            (':', 1.4),
            ('.', 1.3),
            ('=', 1.2),
            ('+', 1.1),
            ('-', 1.0),
            ('*', 0.9),
            ('/', 0.8),
            ('%', 0.7),
            ('|', 0.6),
            ('&', 0.5),
            ('!', 0.4),
            ('?', 0.3),
            ('<', 0.2),
            ('>', 0.1),
            ('_', 0.1),
            ('#', 0.1),
            ('@', 0.1),
            ('$', 0.1),
            ('^', 0.1),
            ('~', 0.1),
            ('\\', 0.1),
            ('`', 0.1),
        ]
        .into_iter()
        .map(|(c, f)| Letter::new(u32::from(c), f))
        .collect()
    }

    /// Sorts letters by decreasing frequency.
    pub fn frequency_order(letters: &[Letter]) -> Vec<Letter> {
        let mut v = letters.to_vec();
        v.sort_by(|a, b| b.f.total_cmp(&a.f).then(a.cp.cmp(&b.cp)));
        v
    }

    /// Sorts letters by weight first, then by decreasing frequency.
    pub fn weighted_frequency_order(
        letters: &[Letter],
        weight: impl Fn(&Letter) -> u32,
    ) -> Vec<Letter> {
        let mut v = letters.to_vec();
        v.sort_by(|a, b| {
            weight(a)
                .cmp(&weight(b))
                .then(b.f.total_cmp(&a.f))
                .then(a.cp.cmp(&b.cp))
        });
        v
    }

    pub fn normalize(letters: Vec<Letter>) -> Vec<Letter> {
        let sum: f64 = letters.iter().map(|l| l.f).sum();
        letters
            .into_iter()
            .map(|l| Letter {
                f: if sum > 0.0 { l.f / sum } else { 0.0 },
                ..l
            })
            .collect()
    }
}

/// Restricts the letters from which words are generated.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    /// Code points of the letters from which to generate words, `None` allows everything.
    pub code_points: Option<HashSet<u32>>,
    /// The letter which must appear in each generated word.
    pub focused: Option<u32>,
}

impl Filter {
    pub fn new(letters: &[u32], focused: Option<u32>) -> Self {
        Self {
            code_points: Some(letters.iter().copied().collect()),
            focused,
        }
    }

    pub fn includes(&self, cp: u32) -> bool {
        self.code_points
            .as_ref()
            .is_none_or(|set| set.contains(&cp))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Entry {
    pub cp: u32,
    pub frequency: u8,
}

/// The Markov chain of a language model.
pub struct TransitionTable {
    pub order: usize,
    pub alphabet: Vec<u32>,
    index: HashMap<u32, usize>,
    offsets: Vec<usize>,
    segments: Vec<Vec<Entry>>,
}

impl TransitionTable {
    pub fn load(data: &[u8]) -> Result<Self> {
        let mut reader = Reader { data, pos: 0 };
        for &c in SIGNATURE {
            ensure!(reader.u8()? == c, "invalid model signature");
        }
        let order = reader.u8()? as usize;
        let size = reader.u8()? as usize;
        ensure!(order >= 1 && size > 0, "invalid model header");
        let mut alphabet = Vec::with_capacity(size);
        for _ in 0..size {
            alphabet.push(reader.u16()? as u32);
        }
        let index: HashMap<u32, usize> = alphabet
            .iter()
            .enumerate()
            .map(|(i, &cp)| (cp, i))
            .collect();
        let offsets = (0..order)
            .map(|i| size.pow((order - i - 1) as u32))
            .collect();
        let count = size.pow(order as u32 - 1);
        let mut segments = Vec::with_capacity(count);
        for _ in 0..count {
            let length = reader.u8()? as usize;
            ensure!(length <= size, "invalid segment length");
            let mut segment = Vec::with_capacity(length);
            for _ in 0..length {
                let i = reader.u8()? as usize;
                ensure!(i < size, "invalid letter index");
                let frequency = reader.u8()?;
                ensure!(frequency > 0, "invalid frequency");
                segment.push(Entry {
                    cp: alphabet[i],
                    frequency,
                });
            }
            segments.push(segment);
        }
        if reader.pos != data.len() {
            bail!("trailing data in model");
        }
        Ok(Self {
            order,
            alphabet,
            index,
            offsets,
            segments,
        })
    }

    /// Returns the possible next letters after the given word prefix.
    pub fn segment(&self, word: &[u32]) -> &[Entry] {
        let order = self.order;
        let mut index = 0;
        for i in 0..order - 1 {
            let pos = word.len() as isize - order as isize + i as isize + 1;
            let cp = if pos >= 0 { word[pos as usize] } else { SPACE };
            match self.index.get(&cp) {
                Some(&j) => index += j * self.offsets[i + 1],
                None => return &[],
            }
        }
        &self.segments[index]
    }

    /// Returns the letters of the model with their relative frequencies.
    pub fn letters(&self, language: &Language) -> Vec<Letter> {
        let mut f: HashMap<u32, f64> = self.alphabet.iter().map(|&cp| (cp, 0.0)).collect();
        for segment in &self.segments {
            for e in segment {
                *f.entry(e.cp).or_default() += e.frequency as f64;
            }
        }
        let letters = self
            .alphabet
            .iter()
            .filter(|&&cp| cp != SPACE)
            .map(|&cp| Letter::with_label(cp, f[&cp], language.letter_name(cp)))
            .collect();
        Letter::normalize(letters)
    }
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn u8(&mut self) -> Result<u8> {
        let v = *self
            .data
            .get(self.pos)
            .ok_or_else(|| anyhow::anyhow!("unexpected end of model"))?;
        self.pos += 1;
        Ok(v)
    }

    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes([self.u8()?, self.u8()?]))
    }
}

/// Word prefixes which end with a given letter, used to quickly generate
/// words which contain the focused letter.
struct PrefixList {
    /// For every letter, the prefixes ending with it and their weights.
    map: HashMap<u32, Vec<(Vec<u32>, u32)>>,
}

impl PrefixList {
    fn new(table: &TransitionTable) -> Self {
        let mut map: HashMap<u32, Vec<(Vec<u32>, u32)>> = HashMap::new();
        let mut word = Vec::new();
        Self::walk(table, &mut word, &mut map);
        Self { map }
    }

    fn walk(
        table: &TransitionTable,
        word: &mut Vec<u32>,
        map: &mut HashMap<u32, Vec<(Vec<u32>, u32)>>,
    ) {
        for e in table.segment(word).to_vec() {
            if e.cp != SPACE {
                word.push(e.cp);
                // keybr.com adds a prefix once per distinct letter in it.
                let weight = word.iter().collect::<HashSet<_>>().len() as u32;
                map.entry(e.cp).or_default().push((word.clone(), weight));
                if word.len() < MIN_LENGTH {
                    Self::walk(table, word, map);
                }
                word.pop();
            }
        }
    }

    fn find(&self, filter: &Filter) -> Vec<(Vec<u32>, u32)> {
        match filter.focused {
            Some(focused) => {
                let prefixes: Vec<_> = self
                    .map
                    .get(&focused)
                    .map(|list| {
                        list.iter()
                            .filter(|(p, _)| p.iter().all(|&cp| filter.includes(cp)))
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_default();
                if prefixes.is_empty() {
                    vec![(vec![focused], 1)]
                } else {
                    prefixes
                }
            }
            None => Vec::new(),
        }
    }
}

/// A phonetic language model which generates pseudo-words.
pub struct PhoneticModel {
    pub language: &'static Language,
    pub letters: Vec<Letter>,
    table: TransitionTable,
    prefixes: PrefixList,
    blacklist: HashSet<String>,
}

impl PhoneticModel {
    pub fn load(language: &'static Language) -> Result<Self> {
        let data = assets::model_data(&language.id)
            .ok_or_else(|| anyhow::anyhow!("no phonetic model for language {}", language.id))?;
        let table = TransitionTable::load(data)?;
        let letters = table.letters(language);
        let prefixes = PrefixList::new(&table);
        let blacklist = assets::blacklist_json(&language.id)
            .map(|json| {
                let words: Vec<String> = serde_json::from_str(json).unwrap_or_default();
                words.iter().map(|w| unscramble_word(w)).collect()
            })
            .unwrap_or_default();
        Ok(Self {
            language,
            letters,
            table,
            prefixes,
            blacklist,
        })
    }

    pub fn table(&self) -> &TransitionTable {
        &self.table
    }

    /// Generates a pseudo-word from the letters allowed by the filter.
    /// Returns an empty string when no word can be generated.
    pub fn next_word(&self, filter: &Filter, rng: &mut dyn Rng) -> String {
        for _ in 0..1000 {
            let word = from_code_points(&self.next_word_raw(filter, rng));
            if self.blacklist.is_empty()
                || !self.blacklist.contains(&self.language.lower_case(&word))
            {
                return word;
            }
        }
        String::new()
    }

    fn next_word_raw(&self, filter: &Filter, rng: &mut dyn Rng) -> Vec<u32> {
        let prefixes = self.prefixes.find(filter);
        let mut word: Vec<u32> = Vec::new();
        let mut attempt = 0;
        let mut retry = |word: &mut Vec<u32>, rng: &mut dyn Rng| -> bool {
            if attempt < 5 {
                attempt += 1;
                word.clear();
                if !prefixes.is_empty() {
                    let (prefix, _) = weighted_random_sample(&prefixes, |(_, w)| *w as f64, rng);
                    word.extend_from_slice(prefix);
                }
                true
            } else {
                false
            }
        };
        retry(&mut word, rng);
        loop {
            let entries: Vec<(u32, f64)> = self
                .table
                .segment(&word)
                .iter()
                .filter(|e| {
                    if e.cp == SPACE {
                        word.len() >= MIN_LENGTH
                    } else {
                        filter.includes(e.cp)
                    }
                })
                .map(|e| {
                    let f = e.frequency as f64;
                    // Boost the space character to generate shorter words.
                    (
                        e.cp,
                        if e.cp == SPACE {
                            f * 1.3f64.powi(word.len() as i32)
                        } else {
                            f
                        },
                    )
                })
                .collect();
            if entries.is_empty() {
                if retry(&mut word, rng) {
                    continue;
                }
                return word;
            }
            let &(cp, _) = weighted_random_sample(&entries, |e| e.1, rng);
            if cp == SPACE {
                return word;
            }
            if word.len() > MAX_LENGTH {
                if retry(&mut word, rng) {
                    continue;
                }
                return word;
            }
            word.push(cp);
        }
    }
}

const SCRAMBLE_X: usize = 23;
const SCRAMBLE_Y: usize = 13;

/// Restores a word from the scrambled blacklist.
pub fn unscramble_word(word: &str) -> String {
    let a: Vec<char> = word.chars().collect();
    let n = a.len();
    (0..n)
        .map(|i| a[(SCRAMBLE_X * i + SCRAMBLE_Y) % n])
        .collect()
}
