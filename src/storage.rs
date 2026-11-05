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
