//! Settings and typing history on disk.
//!
//! Results are appended to a JSON lines file, one lesson per line, so a
//! crash can never lose more than the lesson being typed.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::result::{ResultRecord, TypingResult};
use crate::settings::Settings;

/// The result of a typing test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    /// Milliseconds since the Unix epoch.
    pub time_stamp: i64,
    /// Characters per minute.
    pub speed: f64,
    pub accuracy: f64,
    /// Typed characters.
    pub length: u32,
    /// Milliseconds.
    pub time: u32,
    pub errors: u32,
    /// The duration setting, e.g. `seconds15`.
    pub duration: String,
    /// The text source, e.g. `commonWords`.
    pub source: String,
}

pub struct Storage {
    dir: PathBuf,
}

impl Storage {
    /// Uses `dir`, or `$KEEPER_HOME`, or the platform data directory.
    pub fn open(dir: Option<PathBuf>) -> Result<Self> {
        let dir = match dir.or_else(|| std::env::var_os("KEEPER_HOME").map(PathBuf::from)) {
            Some(dir) => dir,
            None => dirs::data_dir()
                .context("cannot find the data directory")?
                .join("keeper"),
        };
        fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
        Ok(Self { dir })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn settings_path(&self) -> PathBuf {
        self.dir.join("settings.json")
    }

    pub fn results_path(&self) -> PathBuf {
        self.dir.join("results.jsonl")
    }

    pub fn tests_path(&self) -> PathBuf {
        self.dir.join("typing-tests.jsonl")
    }

    /// Loads the settings, or the defaults if there are none. Unreadable
    /// settings are kept aside in `settings.json.bad`.
    pub fn load_settings(&self) -> Settings {
        let path = self.settings_path();
        match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Settings>(&text) {
                Ok(settings) => settings.normalize(),
                Err(_) => {
                    let _ = fs::rename(&path, path.with_extension("json.bad"));
                    Settings::default()
                }
            },
            Err(_) => Settings::default(),
        }
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        let json = serde_json::to_string_pretty(settings)?;
        write_atomic(&self.settings_path(), json.as_bytes())
    }

    /// Loads the lesson results, skipping lines which cannot be parsed.
    pub fn load_results(&self) -> Result<Vec<TypingResult>> {
        let mut results: Vec<TypingResult> = read_lines::<ResultRecord>(&self.results_path())?
            .into_iter()
            .filter_map(|r| r.into_result().recover())
            .collect();
        results.sort_by_key(|r| r.time_stamp);
        Ok(results)
    }

    pub fn append_result(&self, result: &TypingResult) -> Result<()> {
        append_line(&self.results_path(), &result.to_record())
    }

    /// Moves the results into a timestamped backup file and starts afresh.
    pub fn reset_results(&self) -> Result<Option<PathBuf>> {
        let path = self.results_path();
        if !path.exists() {
            return Ok(None);
        }
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let backup = self.dir.join(format!("results-{stamp}.jsonl.bak"));
        fs::rename(&path, &backup)?;
        Ok(Some(backup))
    }

    /// Writes all results as a JSON array, the format keybr.com uses for
    /// downloaded data.
    pub fn export_results(&self, results: &[TypingResult], path: &Path) -> Result<()> {
        let records: Vec<ResultRecord> = results.iter().map(TypingResult::to_record).collect();
        write_atomic(path, serde_json::to_string_pretty(&records)?.as_bytes())
    }

    /// Adds results from a JSON array (such as data downloaded from
    /// keybr.com) or a JSON lines file. Returns the number of new results.
    pub fn import_results(
        &self,
        existing: &[TypingResult],
        path: &Path,
    ) -> Result<Vec<TypingResult>> {
        let text =
            fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        let records: Vec<ResultRecord> = match serde_json::from_str::<Vec<ResultRecord>>(&text) {
            Ok(records) => records,
            Err(_) => text
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(serde_json::from_str)
                .collect::<Result<_, _>>()
                .context("the file is neither a JSON array of results nor JSON lines")?,
        };
        let known: std::collections::HashSet<i64> = existing.iter().map(|r| r.time_stamp).collect();
        let mut added: Vec<TypingResult> = records
            .into_iter()
            .filter_map(|r| r.into_result().recover())
            .filter(|r| !known.contains(&r.time_stamp))
            .collect();
        added.sort_by_key(|r| r.time_stamp);
        added.dedup_by_key(|r| r.time_stamp);
        for result in &added {
            self.append_result(result)?;
        }
        Ok(added)
    }

    pub fn load_tests(&self) -> Result<Vec<TestResult>> {
        read_lines(&self.tests_path())
    }

    pub fn append_test(&self, result: &TestResult) -> Result<()> {
        append_line(&self.tests_path(), result)
    }
}

fn read_lines<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("cannot read {}", path.display())),
    };
    let mut items = Vec::new();
    // Lines are read as bytes, so that one damaged line, even one which
    // is not valid UTF-8, is skipped instead of failing the whole file.
    for line in BufReader::new(file).split(b'\n') {
        if let Ok(item) = serde_json::from_slice(&line?) {
            items.push(item);
        }
    }
    Ok(items)
}

/// Whether the file is not empty and does not end with a line break,
/// which happens when a write was cut short.
fn ends_without_newline(path: &Path) -> Result<bool> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e.into()),
    };
    if file.metadata()?.len() == 0 {
        return Ok(false);
    }
    file.seek(SeekFrom::End(-1))?;
    let mut last = [0u8];
    file.read_exact(&mut last)?;
    Ok(last[0] != b'\n')
}

fn append_line<T: Serialize>(path: &Path, item: &T) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("cannot write {}", path.display()))?;
    let mut line = serde_json::to_string(item)?;
    line.push('\n');
    if ends_without_newline(path)? {
        // Do not append to the remains of a damaged line.
        line.insert(0, '\n');
    }
    file.write_all(line.as_bytes())?;
    file.sync_data()?;
    Ok(())
}

fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, data).with_context(|| format!("cannot write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::result::TextType;
    use crate::settings::LessonType;
    use crate::textinput::{Histogram, Sample};

    fn result(ts: i64) -> TypingResult {
        let histogram = Histogram::new("abc".chars().map(|c| Sample {
            cp: u32::from(c),
            hit_count: 3,
            miss_count: 0,
            time_to_type: 200,
        }));
        TypingResult::new("en-us", TextType::Generated, ts, 100, 20_000, 2, histogram)
    }

    #[test]
    fn settings_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::open(Some(dir.path().to_path_buf())).unwrap();
        assert_eq!(storage.load_settings(), Settings::default());
        let settings = Settings {
            lesson_type: LessonType::Books,
            target_speed: 300,
            ..Settings::default()
        };
        storage.save_settings(&settings).unwrap();
        assert_eq!(storage.load_settings(), settings);
        fs::write(storage.settings_path(), "{nope").unwrap();
        assert_eq!(storage.load_settings(), Settings::default());
        assert!(dir.path().join("settings.json.bad").exists());
    }

    #[test]
    fn results_are_appended_and_survive_garbage() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::open(Some(dir.path().to_path_buf())).unwrap();
        assert!(storage.load_results().unwrap().is_empty());
        storage.append_result(&result(2)).unwrap();
        storage.append_result(&result(1)).unwrap();
        let mut f = OpenOptions::new()
            .append(true)
            .open(storage.results_path())
            .unwrap();
        writeln!(f, "garbage").unwrap();
        let loaded = storage.load_results().unwrap();
        assert_eq!(
            loaded.iter().map(|r| r.time_stamp).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(loaded[0], result(1));
        let backup = storage.reset_results().unwrap().unwrap();
        assert!(backup.exists());
        assert!(storage.load_results().unwrap().is_empty());
        assert!(storage.reset_results().unwrap().is_none());
    }

    #[test]
    fn a_line_of_invalid_utf8_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::open(Some(dir.path().to_path_buf())).unwrap();
        storage.append_result(&result(1)).unwrap();
        let mut f = OpenOptions::new()
            .append(true)
            .open(storage.results_path())
            .unwrap();
        f.write_all(b"\xff\xfe\n").unwrap();
        storage.append_result(&result(2)).unwrap();
        let loaded = storage.load_results().unwrap();
        assert_eq!(loaded.len(), 2);
    }

    #[test]
    fn a_cut_line_does_not_swallow_the_next_result() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::open(Some(dir.path().to_path_buf())).unwrap();
        storage.append_result(&result(1)).unwrap();
        // A write that was cut short, by a full disk or a crash.
        let mut f = OpenOptions::new()
            .append(true)
            .open(storage.results_path())
            .unwrap();
        f.write_all(br#"{"layout":"en-us","textT"#).unwrap();
        storage.append_result(&result(2)).unwrap();
        let loaded = storage.load_results().unwrap();
        assert_eq!(
            loaded.iter().map(|r| r.time_stamp).collect::<Vec<_>>(),
            vec![1, 2]
        );
    }

    #[test]
    fn export_and_import() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::open(Some(dir.path().to_path_buf())).unwrap();
        let file = dir.path().join("export.json");
        storage
            .export_results(&[result(1), result(2)], &file)
            .unwrap();
        let added = storage.import_results(&[result(1)], &file).unwrap();
        assert_eq!(added.len(), 1);
        assert_eq!(storage.load_results().unwrap(), vec![result(2)]);
        // JSON lines work too.
        let lines = dir.path().join("lines.jsonl");
        fs::write(
            &lines,
            serde_json::to_string(&result(3).to_record()).unwrap() + "\n",
        )
        .unwrap();
        assert_eq!(storage.import_results(&[], &lines).unwrap().len(), 1);
        fs::write(&lines, "nope").unwrap();
        assert!(storage.import_results(&[], &lines).is_err());
    }
}
