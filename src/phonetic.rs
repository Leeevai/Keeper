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
