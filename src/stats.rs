//! Statistics for the profile screen, ported from `@keybr/chart` and
//! `@keybr/math`.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use chrono::{Datelike, Duration, NaiveDate};

use crate::assets;
use crate::lesson::Target;
use crate::phonetic::Letter;
use crate::result::{ExpFilter, KeyStatsMap, TypingResult, daily_stats};

/// An empirical distribution of the speeds or accuracies of all keybr.com users.
pub struct Distribution {
    pmf: Vec<f64>,
    cdf: Vec<f64>,
}

impl Distribution {
    fn new(samples: &[f64]) -> Self {
        let total: f64 = samples.iter().sum();
        let mut running = 0.0;
        let mut pmf = Vec::with_capacity(samples.len());
        let mut cdf = Vec::with_capacity(samples.len());
        for s in samples {
            let p = if total > 0.0 { s / total } else { 0.0 };
            running += p;
            pmf.push(p);
            cdf.push(running);
        }
        Self { pmf, cdf }
    }

    /// Typing speeds, indexed by characters per minute.
    pub fn speed() -> &'static Distribution {
        static DIST: OnceLock<Distribution> = OnceLock::new();
        DIST.get_or_init(|| {
            let raw: Vec<f64> =
                serde_json::from_str(assets::DIST_SPEED_JSON).expect("dist_speed.json");
            Distribution::new(&bucketize(&smooth(&smooth(&smooth(&raw, 5), 5), 5), 150))
        })
    }

    /// Accuracies, indexed by accuracy scaled to the length.
    pub fn accuracy() -> &'static Distribution {
        static DIST: OnceLock<Distribution> = OnceLock::new();
        DIST.get_or_init(|| {
            let raw: Vec<f64> =
                serde_json::from_str(assets::DIST_ACCURACY_JSON).expect("dist_accuracy.json");
            Distribution::new(&smooth(&smooth(&smooth(&raw, 5), 5), 5))
        })
    }

    pub fn len(&self) -> usize {
        self.pmf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pmf.is_empty()
    }

    pub fn pmf(&self, index: f64) -> f64 {
        let i = index.round();
        if i < 0.0 || i as usize >= self.len() {
            0.0
        } else {
            self.pmf[i as usize]
        }
    }

    /// The share of users below the index.
    pub fn cdf(&self, index: f64) -> f64 {
        let i = index.round();
        if i < 0.0 {
            0.0
        } else if i as usize >= self.len() {
            1.0
        } else {
            self.cdf[i as usize]
        }
    }

    /// Maps a value from 0 to 1 to an index.
    pub fn scale(&self, value: f64) -> f64 {
        (value * (self.len() as f64 - 1.0))
            .round()
            .clamp(0.0, self.len() as f64 - 1.0)
    }
}

/// A moving average over a window of `wnd` samples on each side.
pub fn smooth(data: &[f64], wnd: usize) -> Vec<f64> {
    (0..data.len())
        .map(|i| {
            let a = i.saturating_sub(wnd);
            let b = (i + wnd + 1).min(data.len());
            data[a..b].iter().sum::<f64>() / (b - a) as f64
        })
        .collect()
}

/// Replaces every sample with the average of its bucket.
pub fn bucketize(data: &[f64], buckets: usize) -> Vec<f64> {
    let mut result = data.to_vec();
    let size = data.len() as f64 / buckets as f64;
    for i in 0..buckets {
        let a = (i as f64 * size).floor() as usize;
        let b = (((i + 1) as f64) * size).floor() as usize;
        if b > a {
            let avg = data[a..b].iter().sum::<f64>() / (b - a) as f64;
            result[a..b].fill(avg);
        }
    }
    result
}

/// Smooths a series with an exponential filter; smoothness goes from 0 (none) to 1.
pub fn smooth_series(values: impl IntoIterator<Item = f64>, smoothness: f64) -> Vec<f64> {
    let mut filter = ExpFilter::new(1.0 / 10f64.powf(smoothness * 3.0));
    values.into_iter().map(|v| filter.add(v)).collect()
}

/// Characters whose statistics the profile shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CharClass {
    #[default]
    Letters,
    Digits,
    Punctuators,
    Specials,
}

impl CharClass {
    pub const ALL: [CharClass; 4] = [
        CharClass::Letters,
        CharClass::Digits,
        CharClass::Punctuators,
        CharClass::Specials,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CharClass::Letters => "Letters",
            CharClass::Digits => "Digits",
            CharClass::Punctuators => "Punctuation characters",
            CharClass::Specials => "Special characters",
        }
    }

    pub fn next(self) -> CharClass {
        let i = Self::ALL.iter().position(|c| *c == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    /// The letters of the class; `letters` are the letters of the language.
    pub fn letters(self, letters: &[Letter]) -> Vec<Letter> {
        match self {
            CharClass::Letters => letters.to_vec(),
            CharClass::Digits => Letter::digits(),
            CharClass::Punctuators => Letter::punctuators(),
            CharClass::Specials => Letter::specials(),
        }
    }
}

/// How often a key was typed and missed.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyUsage {
    pub letter: Letter,
    pub hits: u32,
    pub misses: u32,
    /// Misses per hit, when both are known.
    pub ratio: Option<f64>,
}

pub fn key_usage(stats: &KeyStatsMap) -> Vec<KeyUsage> {
    stats
        .iter()
        .map(|k| {
            let hits: u32 = k.samples.iter().map(|s| s.hit_count).sum();
            let misses: u32 = k.samples.iter().map(|s| s.miss_count).sum();
            let ratio = (hits > 0 && misses > 0).then(|| misses as f64 / hits as f64);
            KeyUsage {
                letter: k.letter.clone(),
                hits,
                misses,
                ratio,
            }
        })
        .collect()
}

/// For every key, the confidence of each lesson resampled to `width`
/// columns; `None` where the key was not typed.
pub fn progress_overview(
    stats: &KeyStatsMap,
    target: &Target,
    width: usize,
) -> Vec<(Letter, Vec<Option<f64>>)> {
    let lessons = stats.result_count;
    stats
        .iter()
        .map(|k| {
            let mut data = vec![None; lessons];
            for s in &k.samples {
                if s.index < lessons {
                    data[s.index] = target.confidence(Some(s.time_to_type));
                }
            }
            let columns = if lessons == 0 {
                vec![None; width]
            } else {
                (0..width)
                    .map(|i| data[(i * lessons / width).min(lessons - 1)])
                    .collect()
            };
            (k.letter.clone(), columns)
        })
        .collect()
}

/// Practice time per day for the calendar, in milliseconds.
pub fn practice_days(results: &[TypingResult]) -> BTreeMap<NaiveDate, u64> {
    daily_stats(results)
        .into_iter()
        .map(|(date, day)| (date, day.stats.time))
        .collect()
}

/// The Monday on or before the date.
pub fn week_start(date: NaiveDate) -> NaiveDate {
    date - Duration::days(date.weekday().num_days_from_monday() as i64)
}
