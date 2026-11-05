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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::result::{KeyStatsMap, TextType};
    use crate::textinput::{Histogram, Sample};

    #[test]
    fn distributions() {
        let speed = Distribution::speed();
        assert_eq!(speed.len(), 751);
        assert!((speed.cdf(10_000.0) - 1.0).abs() < 1e-9);
        assert_eq!(speed.cdf(-1.0), 0.0);
        let total: f64 = (0..speed.len()).map(|i| speed.pmf(i as f64)).sum();
        assert!((total - 1.0).abs() < 1e-9);
        // Typing 40 WPM beats fewer people than typing 80 WPM.
        assert!(speed.cdf(200.0) < speed.cdf(400.0));
        assert!(speed.cdf(200.0) > 0.1 && speed.cdf(400.0) < 1.0);
        let accuracy = Distribution::accuracy();
        assert_eq!(accuracy.len(), 1001);
        assert_eq!(accuracy.scale(0.5), 500.0);
        assert!(accuracy.cdf(accuracy.scale(0.95)) < accuracy.cdf(accuracy.scale(0.99)));
    }

    #[test]
    fn smoothing() {
        assert_eq!(smooth(&[0.0, 3.0, 0.0], 1), vec![1.5, 1.0, 1.5]);
        assert_eq!(
            bucketize(&[1.0, 3.0, 5.0, 7.0], 2),
            vec![2.0, 2.0, 6.0, 6.0]
        );
        assert_eq!(smooth_series([1.0, 3.0], 0.0), vec![1.0, 3.0]);
        let smoothed = smooth_series([0.0, 10.0], 1.0);
        assert!(smoothed[1] < 0.1);
    }

    fn result(index: i64, samples: &[(char, u32, u32, u32)]) -> TypingResult {
        let histogram = Histogram::new(samples.iter().map(|&(c, hit, miss, ttt)| Sample {
            cp: u32::from(c),
            hit_count: hit,
            miss_count: miss,
            time_to_type: ttt,
        }));
        TypingResult::new(
            "en-us",
            TextType::Generated,
            index,
            100,
            20_000,
            0,
            histogram,
        )
    }

    #[test]
    fn usage_and_overview() {
        let letters = vec![
            Letter::new(u32::from('a'), 1.0),
            Letter::new(u32::from('b'), 1.0),
        ];
        let results = vec![
            result(0, &[('a', 10, 2, 300)]),
            result(1, &[('a', 5, 0, 200), ('b', 4, 1, 400)]),
        ];
        let stats = KeyStatsMap::from_results(&letters, &results);
        let usage = key_usage(&stats);
        assert_eq!((usage[0].hits, usage[0].misses), (15, 2));
        assert!((usage[0].ratio.unwrap() - 2.0 / 15.0).abs() < 1e-9);
        assert_eq!((usage[1].hits, usage[1].misses), (4, 1));
        let target = Target {
            target_speed: 200.0,
        };
        let overview = progress_overview(&stats, &target, 4);
        assert_eq!(overview[0].1.len(), 4);
        assert!((overview[0].1[0].unwrap() - 1.0).abs() < 1e-9);
        assert!((overview[0].1[3].unwrap() - 1.5).abs() < 1e-9);
        assert_eq!(overview[1].1[0], None);
        assert!(overview[1].1[3].is_some());
    }
}
