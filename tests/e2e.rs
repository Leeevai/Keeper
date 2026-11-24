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
    /// Everything the program wrote, escape codes included.
    raw: Vec<u8>,
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
        Self::start_with(home, rows, cols, &[])
    }

    /// Starts keeper with extra environment variables.
    fn start_with(home: &Path, rows: u16, cols: u16, env: &[(&str, &str)]) -> Term {
        let pty = native_pty_system();
        let pair = pty.openpty(size(rows, cols)).expect("a pty");
        let mut cmd = CommandBuilder::new(BIN);
        cmd.env("KEEPER_HOME", home);
        cmd.env("TERM", "xterm-256color");
        cmd.env_remove("COLORTERM");
        cmd.env_remove("NO_COLOR");
        for (name, value) in env {
            cmd.env(name, value);
        }
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
            raw: Vec::new(),
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
                Ok(bytes) => {
                    self.raw.extend_from_slice(&bytes);
                    self.parser.process(&bytes);
                }
                Err(_) => break,
            }
            if Instant::now() >= end {
                break;
            }
        }
    }

    /// Feeds the output of the program into the emulator until it pauses.
    /// A frame reaches the terminal in many writes, so reading the screen
    /// before the program goes quiet can find a frame drawn only halfway.
    fn settle(&mut self) {
        let end = Instant::now() + Duration::from_secs(5);
        while Instant::now() < end {
            match self.rx.recv_timeout(Duration::from_millis(150)) {
                Ok(bytes) => {
                    self.raw.extend_from_slice(&bytes);
                    self.parser.process(&bytes);
                }
                Err(_) => break,
            }
        }
    }

    fn screen(&self) -> String {
        self.parser.screen().contents()
    }

    /// Waits for the text to appear, then for the rest of its frame.
    fn wait_for(&mut self, text: &str, timeout: Duration) -> bool {
        let end = Instant::now() + timeout;
        while !self.screen().contains(text) {
            if Instant::now() >= end {
                return false;
            }
            self.pump(Duration::from_millis(50));
        }
        self.settle();
        true
    }

    /// Sends keys and waits for the program to finish redrawing.
    fn send(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).unwrap();
        self.writer.flush().unwrap();
        self.settle();
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

#[test]
fn starts_on_the_practice_screen_and_quits_cleanly() {
    let home = tempfile::tempdir().unwrap();
    let mut term = Term::start(home.path(), 40, 110);
    let screen = term.screen();
    assert!(screen.contains("F1 Practice"), "{screen}");
    assert!(screen.contains("All keys:"), "{screen}");
    assert!(screen.contains("E N I A R L"), "{screen}");
    assert!(
        screen.contains("Guided lessons · English · United States"),
        "{screen}"
    );
    assert!(screen.contains(" Q   W   E   R   T   Y "), "{screen}");
    assert!(term.parser.screen().alternate_screen());
    assert!(term.quit(), "keeper did not exit successfully");
    // The terminal is restored.
    assert!(!term.parser.screen().alternate_screen());
}

#[test]
fn typing_lessons_saves_results_and_updates_the_metrics() {
    let home = tempfile::tempdir().unwrap();
    let mut term = Term::start(home.path(), 40, 110);
    let lesson = term.box_text();
    assert!(lesson.chars().all(|c| "eniarl ".contains(c)), "{lesson}");
    term.type_text(&lesson, Duration::from_millis(55));
    assert!(
        term.wait_for("Speed:", Duration::from_secs(5)),
        "{}",
        term.screen()
    );
    assert!(term.screen().contains("100.0%"), "{}", term.screen());
    assert_eq!(lines_of(&home.path().join("results.jsonl")), 1);
    // A wrong key is an error: the cursor stops, the result is not perfect.
    let lesson = term.box_text();
    term.type_text("#", Duration::from_millis(60));
    term.type_text(&lesson, Duration::from_millis(55));
    assert!(term.wait_for("Accuracy:", Duration::from_secs(5)));
    assert_eq!(lines_of(&home.path().join("results.jsonl")), 2);
    let results = std::fs::read_to_string(home.path().join("results.jsonl")).unwrap();
    let last: serde_json::Value = serde_json::from_str(results.lines().last().unwrap()).unwrap();
    assert_eq!(last["errors"], 1);
    assert_eq!(last["layout"], "en-us");
    assert!(term.quit());

    // The history is loaded again on the next start.
    let mut term = Term::start(home.path(), 40, 110);
    assert!(term.screen().contains("Speed:"), "{}", term.screen());
    assert!(term.quit());
}

#[test]
fn escape_restarts_and_ctrl_n_skips() {
    let home = tempfile::tempdir().unwrap();
    let mut term = Term::start(home.path(), 40, 110);
    let lesson = term.box_text();
    let first: String = lesson.split(' ').next().unwrap().to_string();
    term.type_text(&first, Duration::from_millis(60));
    term.send(ESC);
    assert_eq!(term.box_text(), lesson);
    term.send(b"\x0e"); // Ctrl+N
    term.pump(Duration::from_millis(200));
    assert_ne!(term.box_text(), lesson);
    assert!(term.quit());
}

#[test]
fn practices_your_own_code() {
    let home = tempfile::tempdir().unwrap();
    let code = tempfile::tempdir().unwrap();
    let lines: Vec<String> = (0..30)
        .map(|i| format!("total += item{i}.price * {i};"))
        .collect();
    std::fs::create_dir_all(code.path().join("src")).unwrap();
    std::fs::write(code.path().join("src/cart.js"), lines.join("\n\t")).unwrap();
    // Dependencies and hidden folders are not your code.
    std::fs::create_dir_all(code.path().join("node_modules/dep")).unwrap();
    std::fs::write(
        code.path().join("node_modules/dep/index.js"),
        "dependency();\n",
    )
    .unwrap();
    std::fs::create_dir_all(code.path().join(".git")).unwrap();
    std::fs::write(code.path().join(".git/hook.sh"), "hidden\n").unwrap();
    let settings = serde_json::json!({
        "lesson.type": "files",
        "lesson.files.path": code.path().display().to_string(),
    });
    std::fs::write(home.path().join("settings.json"), settings.to_string()).unwrap();
    let mut term = Term::start(home.path(), 40, 110);
    assert!(term.screen().contains("Your code"), "{}", term.screen());
    let lesson = term.box_text();
    assert!(lesson.starts_with("total += item"), "{lesson}");
    assert!(!lesson.contains("dependency") && !lesson.contains("hidden"));
    term.type_text(&lesson, Duration::from_millis(40));
    assert!(
        term.wait_for("Speed:", Duration::from_secs(5)),
        "{}",
        term.screen()
    );
    let results = std::fs::read_to_string(home.path().join("results.jsonl")).unwrap();
    let result: serde_json::Value = serde_json::from_str(results.lines().next().unwrap()).unwrap();
    assert_eq!(result["textType"], "code", "{results}");
    assert_eq!(result["errors"], 0, "{lesson}");
    assert!(term.quit());

    // A path that cannot be read says so, and practices generated code
    // instead; so does a folder without code.
    std::fs::create_dir(code.path().join("empty")).unwrap();
    for (folder, notice) in [
        ("missing", "Cannot read"),
        ("empty", "No code to practice in"),
    ] {
        let settings = serde_json::json!({
            "lesson.type": "files",
            "lesson.files.path": code.path().join(folder).display().to_string(),
        });
        std::fs::write(home.path().join("settings.json"), settings.to_string()).unwrap();
        let mut term = Term::start(home.path(), 40, 110);
        assert!(term.screen().contains(notice), "{}", term.screen());
        assert!(term.box_text().chars().count() > 20);
        assert!(term.quit());
    }
}

#[test]
fn every_screen_renders() {
    let home = tempfile::tempdir().unwrap();
    let mut term = Term::start(home.path(), 42, 130);
    term.send(F2);
    assert!(
        term.wait_for("Start typing to begin the test.", Duration::from_secs(3)),
        "{}",
        term.screen()
    );
    term.send(F3);
    assert!(
        term.wait_for("All Time Statistics", Duration::from_secs(3)),
        "{}",
        term.screen()
    );
    term.send(PAGE_DOWN);
    term.send(PAGE_DOWN);
    assert!(
        term.wait_for("Typing Speed", Duration::from_secs(3)),
        "{}",
        term.screen()
    );
    term.send(F4);
    assert!(
        term.wait_for("Target typing speed", Duration::from_secs(3)),
        "{}",
        term.screen()
    );
    assert!(
        term.screen().contains("Lesson preview"),
        "{}",
        term.screen()
    );
    term.send(F5);
    assert!(
        term.wait_for("How it works", Duration::from_secs(3)),
        "{}",
        term.screen()
    );
    term.send(TAB);
    assert!(
        term.wait_for("All keys:", Duration::from_secs(3)),
        "{}",
        term.screen()
    );
    assert!(term.quit());
}

#[test]
fn settings_change_the_lesson_and_are_saved() {
    let home = tempfile::tempdir().unwrap();
    let mut term = Term::start(home.path(), 42, 130);
    term.send(F4);
    // Lesson type: guided lessons → common words.
    term.send(RIGHT);
    assert!(term.wait_for("Common words", Duration::from_secs(3)));
    term.send(ESC);
    assert!(
        term.wait_for(
            "Common words · English · United States",
            Duration::from_secs(3)
        ),
        "{}",
        term.screen()
    );
    let settings = std::fs::read_to_string(home.path().join("settings.json")).unwrap();
    assert!(
        settings.contains(r#""lesson.type": "wordlist""#),
        "{settings}"
    );
    assert!(term.quit());
    let mut term = Term::start(home.path(), 42, 130);
    assert!(
        term.screen()
            .contains("Common words · English · United States"),
        "{}",
        term.screen()
    );
    assert!(term.quit());
}

#[test]
fn custom_text_is_edited_with_a_paste_and_practiced() {
    let home = tempfile::tempdir().unwrap();
    let mut term = Term::start(home.path(), 42, 130);
    term.send(F4);
    // Lesson type: guided → common words → books → custom text.
    for _ in 0..3 {
        term.send(RIGHT);
    }
    assert!(
        term.wait_for("Custom text", Duration::from_secs(3)),
        "{}",
        term.screen()
    );
    term.send(DOWN);
    term.send(b"\r");
    assert!(
        term.wait_for("Ctrl+S save", Duration::from_secs(3)),
        "{}",
        term.screen()
    );
    term.send(b"\x15"); // Ctrl+U clears the text.
    term.send(b"\x1b[200~Hello, World!\r\nFrom Keeper.\x1b[201~");
    term.send(b"\x13"); // Ctrl+S saves it.
    assert!(
        term.wait_for("Hello, World! From Keeper.", Duration::from_secs(3)),
        "{}",
        term.screen()
    );
    let settings = std::fs::read_to_string(home.path().join("settings.json")).unwrap();
    assert!(
        settings.contains(r#""lesson.customText.content": "Hello, World!\nFrom Keeper.""#),
        "{settings}"
    );
    term.send(ESC);
    assert!(
        term.wait_for("Custom text · English", Duration::from_secs(3)),
        "{}",
        term.screen()
    );
    let lesson = term.box_text();
    assert!(
        lesson.starts_with("hello world from keeper hello world"),
        "{lesson}"
    );
    term.type_text(&lesson, Duration::from_millis(50));
    assert!(
        term.wait_for("Speed:", Duration::from_secs(5)),
        "{}",
        term.screen()
    );
    let results = std::fs::read_to_string(home.path().join("results.jsonl")).unwrap();
    assert!(results.contains(r#""textType":"natural""#), "{results}");
    assert!(term.quit());
}

#[test]
fn emulates_colemak_on_a_qwerty_system() {
    use keeper::keyboard::{Keyboard, Layout};
    let home = tempfile::tempdir().unwrap();
    std::fs::write(
        home.path().join("settings.json"),
        r#"{"keyboard.layout": "en-colemak"}"#,
    )
    .unwrap();
    let mut term = Term::start(home.path(), 40, 110);
    assert!(
        term.screen().contains("English · Colemak"),
        "{}",
        term.screen()
    );
    // Press the QWERTY keys found at the positions of the Colemak letters.
    let colemak = Keyboard::new(Layout::get("en-colemak"), "ansi101", "standard");
    let qwerty = Keyboard::new(Layout::get("en-us"), "ansi101", "standard");
    let lesson = term.box_text();
    let keys: String = lesson
        .chars()
        .map(|c| {
            let key = colemak
                .combo(u32::from(c))
                .expect("a Colemak key")
                .key
                .clone();
            let typed = qwerty.characters(&key)[0]
                .as_ref()
                .unwrap()
                .code_point()
                .unwrap();
            char::from_u32(typed).unwrap()
        })
        .collect();
    assert_ne!(keys, lesson);
    term.type_text(&keys, Duration::from_millis(50));
    assert!(
        term.wait_for("Speed:", Duration::from_secs(5)),
        "{}",
        term.screen()
    );
    let results = std::fs::read_to_string(home.path().join("results.jsonl")).unwrap();
    let result: serde_json::Value = serde_json::from_str(results.lines().next().unwrap()).unwrap();
    assert_eq!(result["layout"], "en-colemak");
    assert_eq!(result["errors"], 0);
    assert!(term.quit());
}

#[test]
fn practices_code_in_a_language_written_for_keeper() {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(
        home.path().join("settings.json"),
        r#"{"lesson.type": "code", "lesson.code.syntax": "kotlin"}"#,
    )
    .unwrap();
    let mut term = Term::start(home.path(), 40, 110);
    assert!(
        term.screen()
            .contains("Source code · Kotlin · United States"),
        "{}",
        term.screen()
    );
    let lesson = term.box_text();
    assert!(lesson.chars().count() > 20, "{lesson}");
    // Brackets, quotes and operators are typed as shown.
    term.type_text(&lesson, Duration::from_millis(40));
    assert!(
        term.wait_for("Speed:", Duration::from_secs(5)),
        "{}",
        term.screen()
    );
    let results = std::fs::read_to_string(home.path().join("results.jsonl")).unwrap();
    let result: serde_json::Value = serde_json::from_str(results.lines().next().unwrap()).unwrap();
    assert_eq!(result["textType"], "code", "{results}");
    assert_eq!(result["errors"], 0, "{lesson}");
    assert!(term.quit());
}

#[test]
fn practicing_another_language_with_its_own_letters() {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(
        home.path().join("settings.json"),
        r#"{"keyboard.language": "ru", "keyboard.layout": "ru-ru"}"#,
    )
    .unwrap();
    let mut term = Term::start(home.path(), 40, 110);
    assert!(
        term.screen().contains("Russian · Russia"),
        "{}",
        term.screen()
    );
    let lesson = term.box_text();
    assert!(
        lesson
            .chars()
            .all(|c| c == ' ' || ('а'..='я').contains(&c) || c == 'ё'),
        "{lesson}"
    );
    term.type_text(&lesson, Duration::from_millis(55));
    assert!(
        term.wait_for("Speed:", Duration::from_secs(5)),
        "{}",
        term.screen()
    );
    assert_eq!(lines_of(&home.path().join("results.jsonl")), 1);
    assert!(term.quit());
}

#[test]
fn a_typing_test_runs_to_its_report() {
    let home = tempfile::tempdir().unwrap();
    let mut term = Term::start(home.path(), 40, 110);
    term.send(F2);
    // 15 seconds → 30 seconds → one minute → 100 characters.
    for _ in 0..3 {
        term.send(b"\x04");
    }
    assert!(
        term.wait_for("Test: 100 characters", Duration::from_secs(3)),
        "{}",
        term.screen()
    );
    let text = term.box_text();
    term.type_text(&text[..110.min(text.len())], Duration::from_millis(50));
    assert!(
        term.wait_for(
            "Press Enter or Esc to take a new test.",
            Duration::from_secs(5)
        ),
        "{}",
        term.screen()
    );
    assert!(
        term.screen().contains("Characters  100"),
        "{}",
        term.screen()
    );
    assert_eq!(lines_of(&home.path().join("typing-tests.jsonl")), 1);
    term.send(b"\r");
    assert!(term.wait_for("Start typing to begin the test.", Duration::from_secs(3)));
    assert!(term.quit());
}

#[test]
fn a_timed_test_ends_without_more_typing() {
    let home = tempfile::tempdir().unwrap();
    let mut term = Term::start(home.path(), 40, 110);
    term.send(F2);
    let text = term.box_text();
    term.type_text(&text[..20], Duration::from_millis(60));
    assert!(
        term.wait_for("Press Enter or Esc", Duration::from_secs(20)),
        "{}",
        term.screen()
    );
    // 20 characters in 15 seconds is 16 words per minute.
    assert!(term.screen().contains("Time  15s"), "{}", term.screen());
    assert!(
        term.screen().contains("Speed  16.0wpm"),
        "{}",
        term.screen()
    );
    assert!(term.quit());
}

#[test]
fn survives_resizing() {
    let home = tempfile::tempdir().unwrap();
    let mut term = Term::start(home.path(), 40, 110);
    term.resize(16, 60);
    assert!(
        term.wait_for("Guided lessons", Duration::from_secs(3)),
        "{}",
        term.screen()
    );
    assert!(!term.screen().contains(" Q   W   E "), "{}", term.screen());
    term.resize(10, 30);
    term.resize(45, 140);
    assert!(
        term.wait_for(" Q   W   E ", Duration::from_secs(3)),
        "{}",
        term.screen()
    );
    term.send(F3);
    term.resize(12, 40);
    term.send(F4);
    term.send(DOWN);
    term.resize(50, 150);
    assert!(
        term.wait_for("Prefer natural words", Duration::from_secs(3)),
        "{}",
        term.screen()
    );
    assert!(term.quit());
}

/// The row and column of the first character of the lesson text.
fn first_text_cell(term: &Term) -> (u16, u16) {
    let screen = term.screen();
    let lines: Vec<&str> = screen.lines().collect();
    let top = lines
        .iter()
        .position(|l| l.contains('╭'))
        .expect("a text box");
    let left = lines[top].chars().position(|c| c == '╭').unwrap();
    // The text starts after the border and one column of padding.
    (top as u16 + 1, left as u16 + 2)
}

/// Every cell of the screen, as (row, column, cell).
fn cells(term: &Term) -> Vec<(u16, u16, vt100::Cell)> {
    let screen = term.parser.screen();
    let (rows, cols) = screen.size();
    let mut out = Vec::new();
    for row in 0..rows.get() {
        for col in 0..cols.get() {
            if let Some(cell) = screen.cell(row, col) {
                out.push((row, col, cell.clone()));
            }
        }
    }
    out
}

#[test]
fn turns_colors_off_with_no_color() {
    let home = tempfile::tempdir().unwrap();
    let mut term = Term::start_with(home.path(), 40, 110, &[("NO_COLOR", "1")]);
    let colored: Vec<_> = cells(&term)
        .into_iter()
        .filter(|(_, _, c)| {
            c.fgcolor() != vt100::Color::Default || c.bgcolor() != vt100::Color::Default
        })
        .map(|(row, col, _)| (row, col))
        .collect();
    assert!(
        colored.is_empty(),
        "colored cells: {colored:?}\n{}",
        term.screen()
    );
    // The caret, on the first character of the lesson, is in reverse video.
    let lesson = term.box_text();
    let first = lesson.chars().next().unwrap();
    let (row, col) = first_text_cell(&term);
    let caret = term.parser.screen().cell(row, col).unwrap().clone();
    assert_eq!(caret.contents(), first.to_string(), "{}", term.screen());
    assert!(caret.inverse() && !caret.underline(), "{}", term.screen());
    // So is the key to press, below the text.
    let label = first.to_uppercase().to_string();
    let key = cells(&term)
        .into_iter()
        .find(|(r, _, c)| *r > row + 1 && c.contents() == label)
        .expect("the key on the keyboard");
    assert!(key.2.inverse(), "{}", term.screen());
    // A wrong key stops the caret and underlines it.
    let wrong = if first == 'q' { "z" } else { "q" };
    term.type_text(wrong, Duration::from_millis(60));
    term.pump(Duration::from_millis(200));
    let caret = term.parser.screen().cell(row, col).unwrap().clone();
    assert_eq!(caret.contents(), first.to_string());
    assert!(caret.inverse() && caret.underline(), "{}", term.screen());
    assert!(term.quit());
}

#[test]
fn uses_the_basic_colors_on_the_linux_console() {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(home.path().join("settings.json"), r#"{"ui.theme": "dark"}"#).unwrap();
    let mut term = Term::start_with(home.path(), 40, 110, &[("TERM", "linux")]);
    let basic = |c: vt100::Color| match c {
        vt100::Color::Default => true,
        vt100::Color::Idx(i) => i < 16,
        vt100::Color::Rgb(..) => false,
    };
    let mut colors = std::collections::HashSet::new();
    for (row, col, cell) in cells(&term) {
        assert!(
            basic(cell.fgcolor()) && basic(cell.bgcolor()),
            "{row},{col}: {:?} on {:?}",
            cell.fgcolor(),
            cell.bgcolor()
        );
        colors.insert(format!("{:?}", cell.bgcolor()));
    }
    // The keyboard still shows its key colors.
    assert!(colors.len() >= 3, "{colors:?}");
    // They are sent as the basic codes (30 to 37, 90 to 97 and their
    // backgrounds), not as 256-color indexes, which such terminals ignore.
    let raw = String::from_utf8_lossy(&term.raw);
    for indexed in ["[38;5;", ";38;5;", "[48;5;", ";48;5;"] {
        assert!(!raw.contains(indexed), "{indexed} sent");
    }
    let basic_codes = raw
        .split("\x1b[")
        .filter_map(|code| code.split_once('m').map(|(params, _)| params))
        .filter(|params| params.bytes().all(|b| b.is_ascii_digit() || b == b';'))
        .flat_map(|params| params.split(';'))
        .filter_map(|p| p.parse::<u16>().ok())
        .filter(|p| matches!(p, 30..=37 | 90..=97 | 40..=47 | 100..=107))
        .count();
    assert!(basic_codes > 0, "no basic colors");
    assert!(term.quit());
}

#[test]
fn a_chosen_color_mode_wins_over_no_color() {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(
        home.path().join("settings.json"),
        r#"{"ui.theme": "dracula", "ui.colorMode": "truecolor"}"#,
    )
    .unwrap();
    let mut term = Term::start_with(home.path(), 40, 110, &[("NO_COLOR", "1")]);
    // Dracula's background, #282a36.
    let dracula = vt100::Color::Rgb(40, 42, 54);
    let painted = cells(&term)
        .iter()
        .filter(|(_, _, c)| c.bgcolor() == dracula)
        .count();
    assert!(painted > 100, "{painted} cells painted\n{}", term.screen());
    assert!(term.quit());
}

/// macOS keeps file names in UTF-8, so the test runs where it can make one.
#[cfg(target_os = "linux")]
#[test]
fn keeper_code_rejects_paths_that_are_not_utf8() {
    use std::os::unix::ffi::OsStrExt;
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join(std::ffi::OsStr::from_bytes(b"code\xff"));
    std::fs::create_dir(&dir).unwrap();
    let out = Command::new(BIN)
        .arg("code")
        .arg(&dir)
        .env("KEEPER_HOME", home.path())
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success() && err.contains("is not valid UTF-8"),
        "{err}"
    );
}

fn keeper(home: &Path, args: &[&str]) -> (bool, String) {
    let out = Command::new(BIN)
        .args(args)
        .env("KEEPER_HOME", home)
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr),
    )
}

#[test]
fn command_line() {
    let home = tempfile::tempdir().unwrap();
    let (ok, out) = keeper(home.path(), &["path"]);
    assert!(
        ok && out.trim() == home.path().display().to_string(),
        "{out}"
    );
    let (ok, out) = keeper(home.path(), &["stats"]);
    assert!(ok && out.contains("No lessons yet"), "{out}");
    // A keybr.com export, with dates as strings.
    let import = home.path().join("keybr.json");
    std::fs::write(
        &import,
        r#"[{"layout":"en-us","textType":"generated","timeStamp":"2026-11-01T10:00:00.000Z","length":120,
            "time":30000,"errors":2,"speed":240,"histogram":[
            {"codePoint":97,"hitCount":20,"missCount":1,"timeToType":250},
            {"codePoint":101,"hitCount":30,"missCount":0,"timeToType":220},
            {"codePoint":110,"hitCount":15,"missCount":1,"timeToType":300}]}]"#,
    )
    .unwrap();
    let (ok, out) = keeper(home.path(), &["import", import.to_str().unwrap()]);
    assert!(ok && out.contains("Imported 1 new results"), "{out}");
    let (ok, out) = keeper(home.path(), &["import", import.to_str().unwrap()]);
    assert!(ok && out.contains("Imported 0 new results"), "{out}");
    let (ok, out) = keeper(home.path(), &["stats"]);
    assert!(
        ok && out.contains("1 lessons") && out.contains("48.0wpm"),
        "{out}"
    );
    let export = home.path().join("out.json");
    let (ok, out) = keeper(home.path(), &["export", export.to_str().unwrap()]);
    assert!(ok && out.contains("Exported 1 results"), "{out}");
    let exported: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(export).unwrap()).unwrap();
    assert_eq!(exported[0]["histogram"].as_array().unwrap().len(), 3);
    let (ok, _) = keeper(home.path(), &["import", "/nonexistent"]);
    assert!(!ok);
    let (ok, out) = keeper(home.path(), &["code", "/nonexistent"]);
    assert!(!ok && out.contains("cannot open /nonexistent"), "{out}");
    let (ok, out) = keeper(home.path(), &["--help"]);
    assert!(ok && out.contains("typing tutor"), "{out}");
}
