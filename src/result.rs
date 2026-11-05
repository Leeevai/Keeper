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

    pub fn from_stats(layout: &str, text_type: TextType, time_stamp: i64, stats: &Stats) -> Self {
        Self::new(
            layout,
            text_type,
            time_stamp,
            stats.length,
            stats.time,
            stats.errors,
            stats.histogram.clone(),
        )
    }

    /// Results which are too short, too fast or too slow are not saved.
    pub fn validate(&self) -> bool {
        self.length >= 10
            && self.time >= 1000
            && self.complexity >= 1
            && self.speed >= 1.0
            && self.histogram.validate()
    }

    /// Drops implausible samples so that an otherwise valid result can be kept.
    pub fn recover(self) -> Option<Self> {
        if self.validate() {
            return Some(self);
        }
        let histogram = Histogram::new(self.histogram.iter().copied().filter(Sample::is_valid));
        let recovered = Self::new(
            self.layout,
            self.text_type,
            self.time_stamp,
            self.length,
            self.time,
            self.errors,
            histogram,
        );
        recovered.validate().then_some(recovered)
    }

    pub fn local_date(&self) -> NaiveDate {
        local_date(self.time_stamp)
    }

    pub fn to_record(&self) -> ResultRecord {
        ResultRecord {
            layout: self.layout.clone(),
            text_type: self.text_type,
            time_stamp: self.time_stamp,
            length: self.length,
            time: self.time,
            errors: self.errors,
            speed: Some((self.speed * 100.0).round() / 100.0),
            histogram: self.histogram.iter().copied().collect(),
        }
    }
}

pub fn local_date(time_stamp: i64) -> NaiveDate {
    match Local.timestamp_millis_opt(time_stamp).single() {
        Some(t) => t.date_naive(),
        None => DateTime::from_timestamp_millis(time_stamp)
            .unwrap_or_default()
            .date_naive(),
    }
}

/// The serialized form of a result, compatible with the data that
/// keybr.com exports from the profile page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultRecord {
    pub layout: String,
    pub text_type: TextType,
    #[serde(deserialize_with = "deserialize_time_stamp")]
    pub time_stamp: i64,
    pub length: u32,
    pub time: u32,
    pub errors: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
    pub histogram: Vec<Sample>,
}

impl ResultRecord {
    pub fn into_result(self) -> TypingResult {
        TypingResult::new(
            self.layout,
            self.text_type,
            self.time_stamp,
            self.length,
            self.time,
            self.errors,
            Histogram::new(self.histogram),
        )
    }
}

fn deserialize_time_stamp<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Ts {
        Millis(i64),
        Float(f64),
        Text(String),
    }
    match Ts::deserialize(d)? {
        Ts::Millis(v) => Ok(v),
        Ts::Float(v) => Ok(v as i64),
        Ts::Text(s) => DateTime::parse_from_rfc3339(&s)
            .map(|t| t.timestamp_millis())
            .map_err(serde::de::Error::custom),
    }
}
