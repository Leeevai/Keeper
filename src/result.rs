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

/// One lesson worth of statistics for a key.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeySample {
    /// The index of the lesson.
    pub index: usize,
    pub time_stamp: i64,
    pub hit_count: u32,
    pub miss_count: u32,
    pub time_to_type: f64,
    /// The exponentially smoothed time to type.
    pub filtered_time_to_type: f64,
}

/// Exponential smoothing, see <https://en.wikipedia.org/wiki/Exponential_smoothing>.
#[derive(Debug, Clone, Copy)]
pub struct ExpFilter {
    alpha: f64,
    n: usize,
    value: f64,
}

impl ExpFilter {
    pub fn new(alpha: f64) -> Self {
        Self {
            alpha,
            n: 0,
            value: f64::NAN,
        }
    }

    pub fn add(&mut self, v: f64) -> f64 {
        self.n += 1;
        self.value = if self.n > 1 {
            self.alpha * v + (1.0 - self.alpha) * self.value
        } else {
            v
        };
        self.value
    }
}

/// The learning history of a key.
#[derive(Debug, Clone)]
pub struct KeyStats {
    pub letter: Letter,
    pub samples: Vec<KeySample>,
    /// The current smoothed time to type in milliseconds.
    pub time_to_type: Option<f64>,
    /// The best smoothed time to type in milliseconds.
    pub best_time_to_type: Option<f64>,
    filter: ExpFilter,
    index: usize,
}

impl KeyStats {
    pub fn new(letter: Letter) -> Self {
        Self {
            letter,
            samples: Vec::new(),
            time_to_type: None,
            best_time_to_type: None,
            filter: ExpFilter::new(0.1),
            index: 0,
        }
    }

    pub fn append(&mut self, result: &TypingResult) {
        if let Some(sample) = result.histogram.get(self.letter.cp)
            && sample.time_to_type > 0
        {
            let filtered = self.filter.add(sample.time_to_type as f64);
            self.samples.push(KeySample {
                index: self.index,
                time_stamp: result.time_stamp,
                hit_count: sample.hit_count,
                miss_count: sample.miss_count,
                time_to_type: sample.time_to_type as f64,
                filtered_time_to_type: filtered,
            });
            self.time_to_type = Some(filtered);
            self.best_time_to_type = Some(
                self.best_time_to_type
                    .map_or(filtered, |best| best.min(filtered)),
            );
        }
        self.index += 1;
    }
}

/// Statistics of every letter of a lesson.
#[derive(Debug, Clone)]
pub struct KeyStatsMap {
    stats: Vec<KeyStats>,
    index: HashMap<u32, usize>,
    pub result_count: usize,
}

impl KeyStatsMap {
    pub fn new(letters: &[Letter]) -> Self {
        Self {
            stats: letters.iter().cloned().map(KeyStats::new).collect(),
            index: letters.iter().enumerate().map(|(i, l)| (l.cp, i)).collect(),
            result_count: 0,
        }
    }

    pub fn from_results<'a>(
        letters: &[Letter],
        results: impl IntoIterator<Item = &'a TypingResult>,
    ) -> Self {
        let mut map = Self::new(letters);
        for result in results {
            map.append(result);
        }
        map
    }

    pub fn append(&mut self, result: &TypingResult) {
        self.result_count += 1;
        for stats in &mut self.stats {
            stats.append(result);
        }
    }

    pub fn get(&self, cp: u32) -> Option<&KeyStats> {
        self.index.get(&cp).map(|&i| &self.stats[i])
    }

    pub fn iter(&self) -> impl Iterator<Item = &KeyStats> {
        self.stats.iter()
    }
}

/// Last, change, minimum, maximum and average of a metric.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Metric {
    pub last: f64,
    /// The difference between the last value and the average of the previous ones.
    pub delta: f64,
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    count: usize,
    sum: f64,
}

impl Metric {
    pub fn append(&mut self, value: f64) {
        self.count += 1;
        if self.count == 1 {
            *self = Metric {
                last: value,
                delta: value,
                min: value,
                max: value,
                avg: value,
                count: 1,
                sum: value,
            };
        } else {
            self.last = value;
            self.delta = value - self.avg;
            self.sum += value;
            self.min = self.min.min(value);
            self.max = self.max.max(value);
            self.avg = self.sum / self.count as f64;
        }
    }
}
