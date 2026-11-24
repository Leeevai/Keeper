//! Sends the 16 basic colors as the escape codes that every color terminal
//! knows.
//!
//! Crossterm writes every color as an index of the 256-color palette, the
//! basic ones too: dark red is `38;5;1`. Terminals that know only the basic
//! colors, the ones the 16-color mode is for, ignore those codes. While the
//! 16-color mode is on, [`BasicColors`] rewrites them to the basic codes,
//! `31` for dark red, as the output goes to the terminal.

use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const ESC: u8 = 0x1b;

/// Rewrites the parameters of one SGR escape code, the part between
/// `ESC [` and `m`: foreground and background colors 0 to 15 of the 256
/// color palette become the basic codes 30 to 37, 90 to 97, 40 to 47 and
/// 100 to 107. Everything else stays as it was.
pub fn rewrite_sgr(params: &str) -> String {
    let parts: Vec<&str> = params.split(';').collect();
    let mut out: Vec<String> = Vec::with_capacity(parts.len());
    let mut i = 0;
    while i < parts.len() {
        let part = parts[i];
        let index = |at: usize| parts.get(at).and_then(|n| n.parse::<u16>().ok());
        if matches!(part, "38" | "48" | "58") {
            match parts.get(i + 1) {
                Some(&"5") => {
                    match index(i + 2) {
                        Some(n) if n < 16 && part != "58" => {
                            let base = match (part, n < 8) {
                                ("38", true) => 30,
                                ("38", false) => 90 - 8,
                                (_, true) => 40,
                                (_, false) => 100 - 8,
                            };
                            out.push((base + n).to_string());
                        }
                        _ => out.extend(
                            parts[i..(i + 3).min(parts.len())]
                                .iter()
                                .map(|p| p.to_string()),
                        ),
                    }
                    i += 3;
                    continue;
                }
                // A 24-bit color: its three values are not codes of their own.
                Some(&"2") => {
                    out.extend(
                        parts[i..(i + 5).min(parts.len())]
                            .iter()
                            .map(|p| p.to_string()),
                    );
                    i += 5;
                    continue;
                }
                _ => {}
            }
        }
        out.push(part.to_string());
        i += 1;
    }
    out.join(";")
}

/// A writer to the terminal that rewrites the colors of SGR escape codes
/// with [`rewrite_sgr`] while `enabled` is set, and passes everything else
/// through. An escape code split between two writes is held until its end
/// arrives.
pub struct BasicColors<W: Write> {
    inner: W,
    enabled: Arc<AtomicBool>,
    pending: Vec<u8>,
}

impl<W: Write> BasicColors<W> {
    pub fn new(inner: W, enabled: Arc<AtomicBool>) -> Self {
        Self {
            inner,
            enabled,
            pending: Vec::new(),
        }
    }

    /// Rewrites the complete escape codes of `pending`, keeping an
    /// unfinished one at its end for the next write.
    fn drain(&mut self) -> Vec<u8> {
        let input = std::mem::take(&mut self.pending);
        let mut out = Vec::with_capacity(input.len());
        let mut i = 0;
        while i < input.len() {
            if input[i] != ESC {
                out.push(input[i]);
                i += 1;
                continue;
            }
            if i + 1 >= input.len() {
                self.pending = input[i..].to_vec();
                break;
            }
            if input[i + 1] != b'[' {
                out.push(input[i]);
                i += 1;
                continue;
            }
            // A control sequence: parameters, then one final byte.
            let start = i + 2;
            let end = input[start..]
                .iter()
                .position(|b| (0x40..=0x7e).contains(b))
                .map(|p| start + p);
            let Some(end) = end else {
                self.pending = input[i..].to_vec();
                break;
            };
            if input[end] == b'm' {
                let params = String::from_utf8_lossy(&input[start..end]);
                out.extend_from_slice(b"\x1b[");
                out.extend_from_slice(rewrite_sgr(&params).as_bytes());
                out.push(b'm');
            } else {
                out.extend_from_slice(&input[i..=end]);
            }
            i = end + 1;
        }
        out
    }
}

impl<W: Write> Write for BasicColors<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.enabled.load(Ordering::Relaxed) && self.pending.is_empty() {
            return self.inner.write(buf);
        }
        self.pending.extend_from_slice(buf);
        let out = self.drain();
        self.inner.write_all(&out)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let rest = std::mem::take(&mut self.pending);
        self.inner.write_all(&rest)?;
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_colors_get_their_own_codes() {
        assert_eq!(rewrite_sgr("38;5;1"), "31");
        assert_eq!(rewrite_sgr("38;5;9"), "91");
        assert_eq!(rewrite_sgr("48;5;0"), "40");
        assert_eq!(rewrite_sgr("48;5;15"), "107");
        assert_eq!(rewrite_sgr("38;5;6;49"), "36;49");
        assert_eq!(rewrite_sgr("1;38;5;2;48;5;8"), "1;32;100");
        // Other colors and codes stay as they are.
        assert_eq!(rewrite_sgr("38;5;196"), "38;5;196");
        assert_eq!(rewrite_sgr("39;49;0;7"), "39;49;0;7");
        assert_eq!(rewrite_sgr("58;5;3"), "58;5;3");
        // The values of a 24-bit color are not read as codes.
        assert_eq!(rewrite_sgr("48;2;38;5;9"), "48;2;38;5;9");
        assert_eq!(rewrite_sgr("38;5"), "38;5");
        assert_eq!(rewrite_sgr(""), "");
    }

    #[test]
    fn rewrites_the_output_only_while_enabled() {
        let enabled = Arc::new(AtomicBool::new(true));
        let mut w = BasicColors::new(Vec::new(), enabled.clone());
        // An escape code split between writes, around other codes and text.
        w.write_all(b"\x1b[2;5Hab\x1b[38;").unwrap();
        w.write_all(b"5;1mc\x1b").unwrap();
        w.write_all(b"[48;5;12md\xc3\xa9").unwrap();
        w.flush().unwrap();
        assert_eq!(
            String::from_utf8(w.inner.clone()).unwrap(),
            "\x1b[2;5Hab\x1b[31mc\x1b[104mdé"
        );
        enabled.store(false, Ordering::Relaxed);
        w.inner.clear();
        w.write_all(b"\x1b[38;5;1mx").unwrap();
        assert_eq!(w.inner, b"\x1b[38;5;1mx");
    }
}
