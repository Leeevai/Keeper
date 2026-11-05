//! The application: screens, key bindings and the state they share.

pub mod practice;
pub mod profile;
pub mod settings_view;
pub mod typing_test;

use std::time::Instant;

use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::result::{DailyGoal, TypingResult};
use crate::settings::Settings;
use crate::sound::Sounds;
use crate::storage::{Storage, TestResult};
use crate::theme::Theme;
use crate::ui::Styles;
use practice::Practice;
use profile::{Profile, ProfileAction};
use settings_view::{SettingsAction, SettingsView};
use typing_test::TypingTest;

/// What a key press does to the text being typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputAction {
    Char(u32),
    LineBreak,
    ClearChar,
    ClearWord,
}

impl InputAction {
    /// Maps a key press to a typing action, if it is one.
    pub fn from_key(key: &KeyEvent) -> Option<InputAction> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            // Windows reports characters typed with AltGr as Ctrl+Alt.
            KeyCode::Char(c) if ctrl == alt => Some(InputAction::Char(u32::from(c))),
            // Ctrl+W, and Ctrl+Backspace which many terminals send as Ctrl+H.
            KeyCode::Char('w' | 'h') if ctrl => Some(InputAction::ClearWord),
            KeyCode::Backspace if ctrl || alt => Some(InputAction::ClearWord),
            KeyCode::Backspace => Some(InputAction::ClearChar),
            KeyCode::Enter => Some(InputAction::LineBreak),
            _ => None,
        }
    }
}

/// Whether Ctrl is held without Alt: AltGr, reported as Ctrl+Alt on
/// Windows, types characters and must not trigger shortcuts.
fn is_ctrl(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Practice,
    TypingTest,
    Profile,
    Settings,
    Help,
}

impl Screen {
    pub const ALL: [Screen; 5] = [
        Screen::Practice,
        Screen::TypingTest,
        Screen::Profile,
        Screen::Settings,
        Screen::Help,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Screen::Practice => "Practice",
            Screen::TypingTest => "Typing test",
            Screen::Profile => "Profile",
            Screen::Settings => "Settings",
            Screen::Help => "Help",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
}

/// What the screens need to render themselves.
pub struct RenderContext<'a> {
    pub styles: &'a Styles,
    pub settings: &'a Settings,
    pub daily_goal: DailyGoal,
    pub toast: Option<&'a str>,
    /// Milliseconds since the start.
    pub now: f64,
    /// Whether the text cursor should be shown.
    pub focused: bool,
    pub data_dir: std::path::PathBuf,
}

/// Shortens a path in the home directory to `~/...`.
pub fn short_path(path: &std::path::Path) -> String {
    match dirs::home_dir().and_then(|home| path.strip_prefix(home).ok().map(|p| p.to_path_buf())) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

pub struct App {
    pub storage: Storage,
    pub settings: Settings,
    pub results: Vec<TypingResult>,
    pub screen: Screen,
    pub practice: Practice,
    pub typing_test: TypingTest,
    /// Past typing test results.
    pub tests: Vec<TestResult>,
    pub profile: Profile,
    pub settings_view: SettingsView,
    pub styles: Styles,
    pub sounds: Sounds,
    pub help_scroll: u16,
    pub quit: bool,
    start: Instant,
    /// Notifications with the time they expire at.
    toasts: Vec<(String, f64)>,
}

impl App {
    pub fn new(storage: Storage) -> Result<Self> {
        let settings = storage.load_settings();
        let results = storage.load_results()?;
        let tests = storage.load_tests()?;
        let practice = Practice::new(&settings, &results);
        let typing_test = TypingTest::new(&settings);
        let styles = Styles::new(Theme::get(&settings.theme), settings.color_mode);
        Ok(Self {
            storage,
            settings,
            results,
            screen: Screen::Practice,
            practice,
            typing_test,
            tests,
            profile: Profile::default(),
            settings_view: SettingsView::default(),
            styles,
            sounds: Sounds::default(),
            help_scroll: 0,
            quit: false,
            start: Instant::now(),
            toasts: Vec::new(),
        })
    }

    pub fn now_ms(&self, at: Instant) -> f64 {
        at.saturating_duration_since(self.start).as_secs_f64() * 1000.0
    }

    pub fn daily_goal(&self) -> DailyGoal {
        DailyGoal::compute(
            self.settings.daily_goal,
            chrono::Local::now().date_naive(),
            &self.results,
        )
    }

    pub fn notify(&mut self, message: impl Into<String>, at: f64) {
        self.toasts.push((message.into(), at + 6000.0));
    }

    /// The newest notification that has not expired.
    pub fn toast(&self, now: f64) -> Option<&str> {
        self.toasts
            .iter()
            .rev()
            .find(|(_, until)| *until > now)
            .map(|(m, _)| m.as_str())
    }

    /// Changes the settings, saves them and rebuilds what depends on them.
    pub fn apply_settings(&mut self, settings: Settings) {
        let settings = settings.normalize();
        if settings == self.settings {
            return;
        }
        let rebuild = lesson_changed(&self.settings, &settings);
        self.settings = settings;
        if let Err(e) = self.storage.save_settings(&self.settings) {
            let now = self.now_ms(Instant::now());
            self.notify(format!("Could not save the settings: {e}"), now);
        }
        self.styles = Styles::new(Theme::get(&self.settings.theme), self.settings.color_mode);
        if rebuild {
            self.practice = Practice::new(&self.settings, &self.results);
        }
        if !self.typing_test.started() || self.typing_test.report.is_some() {
            self.typing_test = TypingTest::new(&self.settings);
        }
    }

    pub fn handle_event(&mut self, event: Event, at: Instant) {
        match event {
            Event::Key(key) if key.kind != KeyEventKind::Release => self.handle_key(key, at),
            Event::Paste(text) if self.screen == Screen::Settings => {
                self.settings_view.handle_paste(&text)
            }
            _ => {}
        }
    }

    fn handle_key(&mut self, key: KeyEvent, at: Instant) {
        let ctrl = is_ctrl(&key);
        if ctrl && matches!(key.code, KeyCode::Char('c' | 'q')) {
            self.quit = true;
            return;
        }
        // The custom text editor takes every other key.
        if self.screen == Screen::Settings && self.settings_view.editor.is_some() {
            self.settings_key(key);
            return;
        }
        match key.code {
            KeyCode::Tab => {
                self.screen = Screen::ALL[(self.screen.index() + 1) % Screen::ALL.len()];
                return;
            }
            KeyCode::BackTab => {
                let n = Screen::ALL.len();
                self.screen = Screen::ALL[(self.screen.index() + n - 1) % n];
                return;
            }
            KeyCode::F(n) if (1..=Screen::ALL.len() as u8).contains(&n) => {
                self.screen = Screen::ALL[n as usize - 1];
                return;
            }
            _ => {}
        }
        match self.screen {
            Screen::Practice => self.practice_key(key, at),
            Screen::TypingTest => self.typing_test_key(key, at),
            Screen::Profile => self.profile_key(key, at),
            Screen::Settings => self.settings_key(key),
            Screen::Help => self.help_key(key),
        }
    }

    /// Called regularly to end timed tests; returns whether anything changed.
    pub fn tick(&mut self, at: Instant) -> bool {
        let now = self.now_ms(at);
        if self.typing_test.tick(now) {
            self.save_test(now);
            return true;
        }
        false
    }

    /// Whether the screen changes by itself and must be redrawn: a running
    /// test, a key flash, or a notification showing or just gone.
    pub fn animating(&self, at: Instant) -> bool {
        let now = self.now_ms(at);
        let test = self.screen == Screen::TypingTest
            && self.typing_test.started()
            && self.typing_test.report.is_none();
        let flash = self.screen == Screen::Practice
            && self
                .practice
                .last_key
                .as_ref()
                .is_some_and(|(_, _, t)| now - t < 400.0);
        let toast = self.toasts.iter().any(|(_, until)| *until > now - 300.0);
        test || flash || toast
    }

    fn typing_test_key(&mut self, key: KeyEvent, at: Instant) {
        let ctrl = is_ctrl(&key);
        let now = self.now_ms(at);
        if self.typing_test.report.is_some() {
            if matches!(key.code, KeyCode::Enter | KeyCode::Esc) {
                self.typing_test = TypingTest::new(&self.settings);
            }
            return;
        }
        match key.code {
            KeyCode::Esc => self.typing_test = TypingTest::new(&self.settings),
            KeyCode::Char('d') if ctrl => {
                let mut s = self.settings.clone();
                settings_view::Field::TestDuration.change(&mut s, 1);
                self.apply_settings(s);
                self.typing_test = TypingTest::new(&self.settings);
            }
            KeyCode::Char('t') if ctrl => {
                let mut s = self.settings.clone();
                settings_view::Field::TestSource.change(&mut s, 1);
                self.apply_settings(s);
                self.typing_test = TypingTest::new(&self.settings);
            }
            _ => {
                if let Some(action) = InputAction::from_key(&key) {
                    if let Some(feedback) = self.typing_test.handle(action, now) {
                        self.play(feedback);
                    }
                    if self.typing_test.report.is_some() {
                        self.save_test(now);
                    }
                }
            }
        }
    }

    fn play(&mut self, feedback: crate::textinput::Feedback) {
        let s = &self.settings;
        self.sounds
            .play(s.play_sounds, s.sound_theme, s.sound_volume, feedback);
    }

    fn save_test(&mut self, now: f64) {
        if let Some(report) = &self.typing_test.report {
            let result = report.result.clone();
            if result.length > 0 {
                if let Err(e) = self.storage.append_test(&result) {
                    self.notify(format!("Could not save the test: {e}"), now);
                }
                self.tests.push(result);
            }
        }
    }

    fn settings_key(&mut self, key: KeyEvent) {
        match self.settings_view.handle_key(key, &self.settings) {
            SettingsAction::None => {}
            SettingsAction::Back => self.screen = Screen::Practice,
            SettingsAction::Apply(settings) => self.apply_settings(*settings),
        }
    }

    fn practice_key(&mut self, key: KeyEvent, at: Instant) {
        let ctrl = is_ctrl(&key);
        match key.code {
            KeyCode::Esc => self.practice.restart(),
            KeyCode::Left if ctrl => self.practice.restart(),
            KeyCode::Char('r') if ctrl => self.practice.restart(),
            KeyCode::Right if ctrl => self.practice.next_lesson(),
            KeyCode::Char('n') if ctrl => self.practice.next_lesson(),
            KeyCode::Char('v') if ctrl => {
                let settings = Settings {
                    view: self.settings.view.next(),
                    ..self.settings.clone()
                };
                self.apply_settings(settings);
            }
            _ => {
                if let Some(action) = InputAction::from_key(&key) {
                    let now = self.now_ms(at);
                    let feedback = self.practice.handle(action, now);
                    self.play(feedback);
                    if self.practice.completed() {
                        self.finish_lesson(now);
                    }
                }
            }
        }
    }

    /// Saves the result of the completed lesson and starts the next one.
    fn finish_lesson(&mut self, now: f64) {
        let stats = self.practice.stats();
        let result = TypingResult::from_stats(
            &self.practice.lesson.layout().id,
            self.practice.lesson.lesson_type.text_type(),
            chrono::Utc::now().timestamp_millis(),
            &stats,
        );
        if result.validate() {
            let goal_before = self.daily_goal().value;
            if let Err(e) = self.storage.append_result(&result) {
                self.notify(format!("Could not save the result: {e}"), now);
            }
            self.results.push(result.clone());
            for event in self.practice.accept(&result, self.settings.speed_unit) {
                self.notify(event, now);
            }
            if goal_before < 1.0 && self.daily_goal().value >= 1.0 {
                self.notify("Daily goal reached, well done!", now);
            }
        }
        self.practice.next_lesson();
    }

    fn profile_key(&mut self, key: KeyEvent, at: Instant) {
        let now = self.now_ms(at);
        match self.profile.handle_key(key, &self.results) {
            ProfileAction::None => {}
            ProfileAction::Back => self.screen = Screen::Practice,
            ProfileAction::Export => {
                let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
                let dir = dirs::download_dir().unwrap_or_else(|| self.storage.dir().to_path_buf());
                let path = dir.join(format!("keeper-results-{stamp}.json"));
                match self.storage.export_results(&self.results, &path) {
                    Ok(()) => self.notify(
                        format!(
                            "Exported {} results to {}",
                            self.results.len(),
                            short_path(&path)
                        ),
                        now,
                    ),
                    Err(e) => self.notify(format!("Export failed: {e}"), now),
                }
            }
            ProfileAction::Reset => match self.storage.reset_results() {
                Ok(backup) => {
                    self.results.clear();
                    self.practice = Practice::new(&self.settings, &self.results);
                    self.profile = Profile::default();
                    let kept = backup
                        .map(|b| format!(", the old results are in {}", short_path(&b)))
                        .unwrap_or_default();
                    self.notify(format!("Statistics reset{kept}"), now);
                }
                Err(e) => self.notify(format!("Reset failed: {e}"), now),
            },
        }
    }

    fn help_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.screen = Screen::Practice,
            KeyCode::Down | KeyCode::Char('j') => {
                self.help_scroll = self.help_scroll.saturating_add(1)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.help_scroll = self.help_scroll.saturating_sub(1)
            }
            KeyCode::PageDown => self.help_scroll = self.help_scroll.saturating_add(10),
            KeyCode::PageUp => self.help_scroll = self.help_scroll.saturating_sub(10),
            KeyCode::Home => self.help_scroll = 0,
            _ => {}
        }
    }

    /// Whether the terminal cursor is used as a line caret.
    pub fn wants_bar_cursor(&self) -> bool {
        let line = self.settings.caret_shape == crate::settings::CaretShape::Line;
        match self.screen {
            Screen::Practice | Screen::TypingTest => line,
            Screen::Settings => line || self.settings_view.editor.is_some(),
            _ => false,
        }
    }
}

/// Drops the last hints until the rest fits in the width.
fn fit_hints(hints: &str, width: u16) -> String {
    let mut parts: Vec<&str> = hints.split(" · ").collect();
    while parts.len() > 1
        && unicode_width::UnicodeWidthStr::width(parts.join(" · ").as_str()) > width as usize
    {
        parts.pop();
    }
    parts.join(" · ")
}

/// Whether the lessons must be regenerated after a settings change.
fn lesson_changed(a: &Settings, b: &Settings) -> bool {
    let strip = |s: &Settings| Settings {
        theme: String::new(),
        color_mode: Default::default(),
        speed_unit: Default::default(),
        view: Default::default(),
        caret_shape: Default::default(),
        whitespace_style: Default::default(),
        play_sounds: Default::default(),
        sound_volume: 0.0,
        sound_theme: Default::default(),
        keyboard_colors: true,
        keyboard_pointers: true,
        daily_goal: 0,
        test_source: Default::default(),
        test_language: String::new(),
        test_word_list_size: 0,
        test_book: String::new(),
        test_paragraph_index: 0,
        test_duration: Default::default(),
        ..s.clone()
    };
    strip(a) != strip(b)
}
