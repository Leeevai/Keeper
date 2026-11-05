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
