//! End-to-end tests: the real binary runs in a pseudo terminal, keys are
//! typed into it and the screen is read back through a terminal emulator.

use std::io::{Read, Write};
use std::num::NonZeroU16;
use std::path::Path;
use std::process::Command;
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

const BIN: &str = env!("CARGO_BIN_EXE_keeper");

struct Term {
    parser: vt100::Parser,
    rx: Receiver<Vec<u8>>,
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
}

fn size(rows: u16, cols: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

impl Term {
    fn start(home: &Path, rows: u16, cols: u16) -> Term {
        let pty = native_pty_system();
        let pair = pty.openpty(size(rows, cols)).expect("a pty");
        let mut cmd = CommandBuilder::new(BIN);
        cmd.env("KEEPER_HOME", home);
        cmd.env("TERM", "xterm-256color");
        cmd.env_remove("COLORTERM");
        let child = pair.slave.spawn_command(cmd).expect("keeper starts");
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().expect("a reader");
        let writer = pair.master.take_writer().expect("a writer");
        let (tx, rx) = channel();
        thread::spawn(move || {
            let mut buf = [0u8; 8192];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        let nz = |v| NonZeroU16::new(v).unwrap();
        let mut term = Term {
            parser: vt100::Parser::new(nz(rows), nz(cols), 0),
            rx,
            writer,
            master: pair.master,
            child,
        };
        assert!(
            term.wait_for("Keeper", Duration::from_secs(10)),
            "keeper did not start:\n{}",
            term.screen()
        );
        term
    }

    /// Feeds the output of the program into the emulator for a while.
    fn pump(&mut self, duration: Duration) {
        let end = Instant::now() + duration;
        loop {
            let left = end.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(bytes) => self.parser.process(&bytes),
                Err(_) => break,
            }
            if Instant::now() >= end {
                break;
            }
        }
    }

    fn screen(&self) -> String {
        self.parser.screen().contents()
    }

    fn wait_for(&mut self, text: &str, timeout: Duration) -> bool {
        let end = Instant::now() + timeout;
        while Instant::now() < end {
            if self.screen().contains(text) {
                return true;
            }
            self.pump(Duration::from_millis(50));
        }
        self.screen().contains(text)
    }

    fn send(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).unwrap();
        self.writer.flush().unwrap();
        self.pump(Duration::from_millis(60));
    }

    /// Types every character with a pause, like a person would.
    fn type_text(&mut self, text: &str, gap: Duration) {
        for ch in text.chars() {
            let mut buf = [0u8; 4];
            self.writer
                .write_all(ch.encode_utf8(&mut buf).as_bytes())
                .unwrap();
            self.writer.flush().unwrap();
            self.pump(gap);
        }
    }

    /// Reads the text inside the rounded box on the screen.
    fn box_text(&self) -> String {
        let screen = self.screen();
        let lines: Vec<&str> = screen.lines().collect();
        let top = lines
            .iter()
            .position(|l| l.contains('╭'))
            .expect("a text box");
        let bottom = lines
            .iter()
            .skip(top + 1)
            .position(|l| l.contains('╰'))
            .expect("a box bottom")
            + top
            + 1;
        let left = lines[top].chars().position(|c| c == '╭').unwrap();
        lines[top + 1..bottom]
            .iter()
            .map(|l| {
                let inner: String = l.chars().skip(left + 1).take_while(|c| *c != '│').collect();
                inner.replace('·', " ").trim().to_string()
            })
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn resize(&mut self, rows: u16, cols: u16) {
        self.master.resize(size(rows, cols)).unwrap();
        self.parser.set_size(
            NonZeroU16::new(rows).unwrap(),
            NonZeroU16::new(cols).unwrap(),
        );
        self.pump(Duration::from_millis(300));
    }

    fn quit(&mut self) -> bool {
        self.send(b"\x11"); // Ctrl+Q
        let end = Instant::now() + Duration::from_secs(5);
        while Instant::now() < end {
            if let Ok(Some(status)) = self.child.try_wait() {
                self.pump(Duration::from_millis(100));
                return status.success();
            }
            self.pump(Duration::from_millis(50));
        }
        false
    }
}

impl Drop for Term {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

const F2: &[u8] = b"\x1bOQ";
const F3: &[u8] = b"\x1bOR";
const F4: &[u8] = b"\x1bOS";
const F5: &[u8] = b"\x1b[15~";
const TAB: &[u8] = b"\t";
const ESC: &[u8] = b"\x1b";
const RIGHT: &[u8] = b"\x1b[C";
const DOWN: &[u8] = b"\x1b[B";
const PAGE_DOWN: &[u8] = b"\x1b[6~";

fn lines_of(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .map(|s| s.lines().count())
        .unwrap_or(0)
}
