//! Lesson results and statistics, ported from `@keybr/result`.

use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Local, NaiveDate, TimeZone};
use serde::{Deserialize, Deserializer, Serialize};

use crate::phonetic::Letter;
use crate::textinput::{Histogram, Sample, Stats};

/// The kind of text a lesson was made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextType {
    Generated,
    Natural,
    Numbers,
    Code,
}

/// Converts a time to type a character in milliseconds into characters per minute.
pub fn time_to_speed(ms: f64) -> f64 {
    60_000.0 / ms
}

/// Converts characters per minute into the time to type a character in milliseconds.
pub fn speed_to_time(cpm: f64) -> f64 {
    60_000.0 / cpm
}

/// The result of a completed lesson.
#[derive(Debug, Clone, PartialEq)]
pub struct TypingResult {
    pub layout: String,
    pub text_type: TextType,
    /// Milliseconds since the Unix epoch.
    pub time_stamp: i64,
    /// The number of typed characters.
    pub length: u32,
    /// Milliseconds.
    pub time: u32,
    pub errors: u32,
    pub histogram: Histogram,
    /// The number of distinct characters.
    pub complexity: usize,
    /// Characters per minute.
    pub speed: f64,
    pub accuracy: f64,
    pub score: f64,
}

impl TypingResult {
    pub fn new(
        layout: impl Into<String>,
        text_type: TextType,
        time_stamp: i64,
        length: u32,
        time: u32,
        errors: u32,
        histogram: Histogram,
    ) -> Self {
        let complexity = histogram.complexity();
        let (mut speed, mut accuracy, mut score) = (0.0, 0.0, 0.0);
        if length > 0 && time > 0 && complexity > 0 {
            speed = length as f64 / (time as f64 / 1000.0) * 60.0;
            accuracy = (length as f64 - errors as f64) / length as f64;
            score = speed * complexity as f64 / (errors as f64 + 1.0) * (length as f64 / 50.0);
        }
        Self {
            layout: layout.into(),
            text_type,
            time_stamp,
            length,
            time,
            errors,
            histogram,
            complexity,
            speed,
            accuracy,
            score,
        }
    }
}
