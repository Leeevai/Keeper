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
