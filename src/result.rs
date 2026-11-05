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

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SummaryStats {
    pub count: usize,
    /// Total milliseconds.
    pub time: u64,
    pub speed: Metric,
    pub accuracy: Metric,
    pub score: Metric,
}

impl SummaryStats {
    pub fn from_results<'a>(results: impl IntoIterator<Item = &'a TypingResult>) -> Self {
        let mut stats = Self::default();
        for result in results {
            stats.append(result);
        }
        stats
    }

    pub fn append(&mut self, result: &TypingResult) {
        self.count += 1;
        self.time += result.time as u64;
        self.speed.append(result.speed);
        self.accuracy.append(result.accuracy);
        self.score.append(result.score);
    }
}

/// A lesson in an accuracy streak.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StreakItem {
    pub time_stamp: i64,
    pub speed: f64,
    pub length: u32,
    pub accuracy: f64,
}

impl From<&TypingResult> for StreakItem {
    fn from(r: &TypingResult) -> Self {
        Self {
            time_stamp: r.time_stamp,
            speed: r.speed,
            length: r.length,
            accuracy: r.accuracy,
        }
    }
}

/// A run of consecutive lessons with accuracy at or above a level.
#[derive(Debug, Clone, PartialEq)]
pub struct Streak {
    pub level: f64,
    pub results: Vec<StreakItem>,
}

/// Tracks the current accuracy streaks at 100%, 97% and 95%.
///
/// A port of the linked list of streaks in keybr.com: a result accepted by
/// a higher level also counts towards the lower levels, and the lower level
/// takes the results over when the higher level streak breaks.
#[derive(Debug, Clone)]
pub struct StreakList {
    /// Index 0 is the top sentinel, then 100%, 97%, 95%, and the bottom.
    levels: [f64; 5],
    results: [Vec<StreakItem>; 5],
}

impl Default for StreakList {
    fn default() -> Self {
        Self {
            levels: [f64::INFINITY, 1.0, 0.97, 0.95, 0.0],
            results: Default::default(),
        }
    }
}

impl StreakList {
    pub const LEVELS: [f64; 3] = [1.0, 0.97, 0.95];

    pub fn append(&mut self, result: &TypingResult) {
        self.append_impl(4, StreakItem::from(result), &mut |_| {});
    }

    /// The current streaks at 100%, 97% and 95%.
    pub fn streaks(&self) -> Vec<Streak> {
        (1..=3)
            .map(|i| Streak {
                level: self.levels[i],
                results: self.results[i].clone(),
            })
            .collect()
    }

    fn append_impl(&mut self, i: usize, item: StreakItem, accept: &mut dyn FnMut(Streak)) -> bool {
        if i == 0 {
            return false;
        }
        let next = i - 1;
        let started = self.results[i].len() > self.results[next].len();
        if self.append_impl(next, item, accept) {
            if started {
                self.results[i].push(item);
            }
            return true;
        }
        if !started {
            let taken = self.results[next].clone();
            self.results[i].extend(taken);
        }
        self.results[next].clear();
        if item.accuracy >= self.levels[i] {
            self.results[i].push(item);
            return true;
        }
        if started && !self.results[i].is_empty() {
            accept(Streak {
                level: self.levels[i],
                results: self.results[i].clone(),
            });
        }
        false
    }

    fn end_impl(&self, i: usize, accept: &mut dyn FnMut(Streak)) {
        if i > 1 {
            self.end_impl(i - 1, accept);
        }
        if !self.results[i].is_empty() {
            accept(Streak {
                level: self.levels[i],
                results: self.results[i].clone(),
            });
        }
    }

    /// Finds the longest streak at every accuracy level.
    pub fn find_longest<'a>(results: impl IntoIterator<Item = &'a TypingResult>) -> Vec<Streak> {
        let mut candidates: Vec<Streak> = Self::LEVELS
            .iter()
            .map(|&level| Streak {
                level,
                results: Vec::new(),
            })
            .collect();
        let mut accept = |streak: Streak| {
            for candidate in candidates.iter_mut() {
                if streak.level >= candidate.level
                    && streak.results.len() >= candidate.results.len()
                {
                    candidate.results = streak.results;
                    break;
                }
            }
        };
        let mut list = StreakList::default();
        for result in results {
            list.append_impl(4, StreakItem::from(result), &mut accept);
        }
        list.end_impl(4, &mut accept);
        candidates
            .into_iter()
            .filter(|s| !s.results.is_empty())
            .collect()
    }
}

/// Statistics for a calendar day.
#[derive(Debug, Clone)]
pub struct DailyStats {
    pub date: NaiveDate,
    pub stats: SummaryStats,
}

/// Groups results by their local date.
pub fn daily_stats<'a>(
    results: impl IntoIterator<Item = &'a TypingResult>,
) -> BTreeMap<NaiveDate, DailyStats> {
    let mut map: BTreeMap<NaiveDate, DailyStats> = BTreeMap::new();
    for result in results {
        let date = result.local_date();
        map.entry(date)
            .or_insert_with(|| DailyStats {
                date,
                stats: SummaryStats::default(),
            })
            .stats
            .append(result);
    }
    map
}

/// How much of the daily goal is done.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DailyGoal {
    /// Minutes, zero if not set.
    pub goal: u32,
    /// Progress where 1.0 is done.
    pub value: f64,
}

impl DailyGoal {
    pub fn compute<'a>(
        goal: u32,
        today: NaiveDate,
        results: impl IntoIterator<Item = &'a TypingResult>,
    ) -> Self {
        let time: u64 = results
            .into_iter()
            .filter(|r| r.local_date() == today)
            .map(|r| r.time as u64)
            .sum();
        Self {
            goal,
            value: Self::measure(goal, time),
        }
    }

    pub fn measure(goal: u32, time_ms: u64) -> f64 {
        if goal > 0 {
            time_ms as f64 / 1000.0 / 60.0 / goal as f64
        } else {
            0.0
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn result(
        time_stamp: i64,
        speed_cpm: f64,
        accuracy: f64,
        samples: &[(char, u32)],
    ) -> TypingResult {
        let length = 100u32;
        let time = (length as f64 / speed_cpm * 60_000.0).round() as u32;
        let errors = ((1.0 - accuracy) * length as f64).round() as u32;
        let histogram = Histogram::new(samples.iter().map(|&(c, ttt)| Sample {
            cp: u32::from(c),
            hit_count: 10,
            miss_count: 0,
            time_to_type: ttt,
        }));
        TypingResult::new(
            "en-us",
            TextType::Generated,
            time_stamp,
            length,
            time,
            errors,
            histogram,
        )
    }

    #[test]
    fn computes_speed_accuracy_and_score() {
        let r = result(0, 300.0, 0.98, &[('a', 200), ('b', 200), ('c', 200)]);
        assert!((r.speed - 300.0).abs() < 0.1);
        assert!((r.accuracy - 0.98).abs() < 1e-9);
        assert_eq!(r.complexity, 3);
        let expected = r.speed * 3.0 / 3.0 * 2.0;
        assert!((r.score - expected).abs() < 1e-6);
        assert!(r.validate());
        assert!(!result(0, 300.0, 1.0, &[('a', 200)]).validate());
    }

    #[test]
    fn recovers_results_with_implausible_samples() {
        let r = result(
            0,
            300.0,
            1.0,
            &[('a', 200), ('b', 200), ('c', 200), ('d', 5)],
        );
        assert!(!r.validate());
        let recovered = r.recover().unwrap();
        assert_eq!(recovered.complexity, 3);
        assert!(result(0, 300.0, 1.0, &[('a', 5)]).recover().is_none());
    }

    #[test]
    fn records_round_trip_and_accept_keybr_exports() {
        let r = result(
            1_700_000_000_000,
            250.0,
            0.9,
            &[('a', 200), ('b', 300), ('c', 400)],
        );
        let json = serde_json::to_string(&r.to_record()).unwrap();
        let back: ResultRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back.into_result(), r);
        let keybr = r#"{"layout":"en-us","textType":"generated","timeStamp":"2023-11-14T22:13:20.000Z",
            "length":100,"time":24000,"errors":10,"speed":250,
            "histogram":[{"codePoint":97,"hitCount":10,"missCount":0,"timeToType":200}]}"#;
        let imported: ResultRecord = serde_json::from_str(keybr).unwrap();
        assert_eq!(imported.time_stamp, 1_700_000_000_000);
    }

    #[test]
    fn key_stats_are_smoothed() {
        let letters = vec![
            Letter::new(u32::from('a'), 1.0),
            Letter::new(u32::from('b'), 0.5),
        ];
        let mut map = KeyStatsMap::new(&letters);
        map.append(&result(0, 300.0, 1.0, &[('a', 400)]));
        map.append(&result(1, 300.0, 1.0, &[('a', 200)]));
        map.append(&result(2, 300.0, 1.0, &[('b', 300)]));
        let a = map.get(u32::from('a')).unwrap();
        assert_eq!(a.samples.len(), 2);
        assert_eq!(a.samples[1].index, 1);
        // 0.1 * 200 + 0.9 * 400
        assert!((a.time_to_type.unwrap() - 380.0).abs() < 1e-9);
        assert!((a.best_time_to_type.unwrap() - 380.0).abs() < 1e-9);
        let b = map.get(u32::from('b')).unwrap();
        assert_eq!(b.samples[0].index, 2);
        assert!(map.get(u32::from('z')).is_none());
        assert_eq!(map.result_count, 3);
    }

    #[test]
    fn summary_metrics() {
        let mut m = Metric::default();
        m.append(10.0);
        assert_eq!((m.last, m.delta, m.avg), (10.0, 10.0, 10.0));
        m.append(20.0);
        assert_eq!(
            (m.last, m.delta, m.min, m.max, m.avg),
            (20.0, 10.0, 10.0, 20.0, 15.0)
        );
    }

    fn acc(values: &[f64]) -> Vec<TypingResult> {
        values
            .iter()
            .enumerate()
            .map(|(i, &a)| result(i as i64, 300.0, a, &[('a', 200), ('b', 200), ('c', 200)]))
            .collect()
    }

    #[test]
    fn current_streaks() {
        let mut list = StreakList::default();
        for r in &acc(&[1.0, 0.98, 0.96]) {
            list.append(r);
        }
        let streaks = list.streaks();
        assert_eq!(streaks[0].results.len(), 0);
        assert_eq!(streaks[1].results.len(), 0);
        assert_eq!(streaks[2].results.len(), 3);
        list.append(&acc(&[1.0])[0]);
        let streaks = list.streaks();
        // The new lesson starts a 100% streak and extends the 95% one.
        assert_eq!(streaks[0].results.len(), 1);
        assert_eq!(streaks[1].results.len(), 0);
        assert_eq!(streaks[2].results.len(), 4);
    }

    #[test]
    fn longest_streaks() {
        let results = acc(&[
            1.0, 1.0, 0.5, 1.0, 0.98, 0.98, 0.98, 0.5, 0.96, 0.96, 0.96, 0.96, 0.96,
        ]);
        let longest = StreakList::find_longest(&results);
        let lengths: Vec<(f64, usize)> =
            longest.iter().map(|s| (s.level, s.results.len())).collect();
        assert_eq!(lengths, vec![(1.0, 2), (0.97, 4), (0.95, 5)]);
        assert!(StreakList::find_longest(&acc(&[0.5, 0.6])).is_empty());
    }

    #[test]
    fn daily_stats_and_goal() {
        let day = 86_400_000;
        let results = vec![
            result(0, 300.0, 1.0, &[('a', 200)]),
            result(day * 3, 300.0, 1.0, &[('a', 200)]),
        ];
        let days = daily_stats(&results);
        assert_eq!(days.len(), 2);
        let today = results[1].local_date();
        let goal = DailyGoal::compute(1, today, &results);
        assert!((goal.value - results[1].time as f64 / 60_000.0).abs() < 1e-9);
        assert_eq!(DailyGoal::compute(0, today, &results).value, 0.0);
    }
}
