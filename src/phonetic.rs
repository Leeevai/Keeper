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
}
