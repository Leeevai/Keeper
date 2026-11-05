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
