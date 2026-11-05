//! Keyboard layouts and geometries, ported from `@keybr/keyboard`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::OnceLock;

use serde::Deserialize;

use crate::assets;
use crate::language::Language;
use crate::unicode::{combine_diacritic, is_diacritic};

pub type KeyId = String;

/// The characters of every key, in the order of the original layout.
pub type KeyCharacters = Vec<(KeyId, Vec<Option<Character>>)>;

/// A character produced by a key.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Character {
    CodePoint(u32),
    Dead { dead: u32 },
    Special { special: u32 },
    Ligature { ligature: String },
}

impl Character {
    pub fn code_point(&self) -> Option<u32> {
        match self {
            Character::CodePoint(cp) => Some(*cp),
            _ => None,
        }
    }

    /// A short label to print on a key cap.
    pub fn label(&self) -> String {
        match self {
            Character::CodePoint(cp) => crate::unicode::cp_to_string(*cp),
            Character::Dead { dead } => format!("◌{}", crate::unicode::cp_to_string(*dead)),
            Character::Special { .. } => "·".into(),
            Character::Ligature { ligature } => ligature.clone(),
        }
    }
}
