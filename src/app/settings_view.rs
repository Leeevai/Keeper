//! The settings screen, with every option of keybr.com.

use std::cell::RefCell;

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};

use super::RenderContext;
use crate::code::{FLAGS, Syntax};
use crate::content::Book;
use crate::keyboard::{Keyboard, Layout as KeyboardLayout, zone_mods};
use crate::lesson::{Lesson, shared_model};
use crate::result::TypingResult;
use crate::rng::Lcg;
use crate::settings::{CaretShape, Settings, WhitespaceStyle};
use crate::textinput::{Attr, Char, flatten};
use crate::ui::keyboard::KeyboardView;
use crate::ui::text::TextView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    LessonType,
    NaturalWords,
    KeyboardOrder,
    AlphabetSize,
    RecoverKeys,
    WordListSize,
    LongWordsOnly,
    Book,
    ParagraphIndex,
    BookLettersOnly,
    BookLowercase,
    CustomText,
    CustomLettersOnly,
    CustomLowercase,
    CustomRandomize,
    CodeSyntax,
    CodeFlag(usize),
    Benford,
    Capitals,
    Punctuators,
    RepeatWords,
    LessonLength,
    TargetSpeed,
    DailyGoal,
    StopOnError,
    ForgiveErrors,
    SpaceSkipsWords,
    CaretShape,
    Whitespace,
    PlaySounds,
    SoundTheme,
    SoundVolume,
    Language,
    Layout,
    Geometry,
    Zones,
    Emulation,
    SystemLayout,
    KeyboardColors,
    KeyboardPointers,
    SpeedUnit,
    Theme,
    ColorMode,
    View,
    TestDuration,
    TestSource,
    TestLanguage,
    TestWordListSize,
    TestBook,
    TestParagraph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Lessons,
    Typing,
    Keyboard,
    Miscellaneous,
    TypingTest,
}

impl Section {
    fn title(self) -> &'static str {
        match self {
            Section::Lessons => "Lessons",
            Section::Typing => "Typing",
            Section::Keyboard => "Keyboard",
            Section::Miscellaneous => "Miscellaneous",
            Section::TypingTest => "Typing test",
        }
    }
}

/// The visible fields for the settings, in display order.
pub fn fields(s: &Settings) -> Vec<(Section, Field)> {
    use Field::*;
    let mut v = vec![(Section::Lessons, LessonType)];
    match s.lesson_type {
        crate::settings::LessonType::Guided => {
            v.extend(
                [NaturalWords, KeyboardOrder, AlphabetSize, RecoverKeys]
                    .map(|f| (Section::Lessons, f)),
            );
        }
        crate::settings::LessonType::Wordlist => {
            v.extend([WordListSize, LongWordsOnly].map(|f| (Section::Lessons, f)));
        }
        crate::settings::LessonType::Books => {
            v.extend(
                [Book, ParagraphIndex, BookLettersOnly, BookLowercase]
                    .map(|f| (Section::Lessons, f)),
            );
        }
        crate::settings::LessonType::Custom => {
            v.extend(
                [
                    CustomText,
                    CustomLettersOnly,
                    CustomLowercase,
                    CustomRandomize,
                ]
                .map(|f| (Section::Lessons, f)),
            );
        }
        crate::settings::LessonType::Code => {
            v.push((Section::Lessons, CodeSyntax));
            let syntax = Syntax::get(&s.code_syntax);
            for (i, flag) in FLAGS.iter().enumerate() {
                if syntax.flags.contains(flag) {
                    v.push((Section::Lessons, CodeFlag(i)));
                }
            }
        }
        crate::settings::LessonType::Numbers => v.push((Section::Lessons, Benford)),
    }
    if matches!(
        s.lesson_type,
        crate::settings::LessonType::Guided | crate::settings::LessonType::Wordlist
    ) {
        v.extend([Capitals, Punctuators, RepeatWords].map(|f| (Section::Lessons, f)));
    }
    v.extend([LessonLength, TargetSpeed, DailyGoal].map(|f| (Section::Lessons, f)));
    v.extend(
        [
            StopOnError,
            ForgiveErrors,
            SpaceSkipsWords,
            CaretShape,
            Whitespace,
            PlaySounds,
        ]
        .map(|f| (Section::Typing, f)),
    );
    if s.play_sounds != crate::settings::PlaySounds::None {
        v.extend([SoundTheme, SoundVolume].map(|f| (Section::Typing, f)));
    }
    v.extend([Language, Layout, Geometry, Zones, Emulation].map(|f| (Section::Keyboard, f)));
    if s.emulation == crate::settings::Emulation::Forward && s.keyboard_options().layout.emulate {
        v.push((Section::Keyboard, SystemLayout));
    }
    v.extend([KeyboardColors, KeyboardPointers].map(|f| (Section::Keyboard, f)));
    v.extend([SpeedUnit, Theme, ColorMode, View].map(|f| (Section::Miscellaneous, f)));
    v.extend([TestDuration, TestSource].map(|f| (Section::TypingTest, f)));
    match s.test_source {
        crate::settings::TestSource::Book => {
            v.extend([TestBook, TestParagraph].map(|f| (Section::TypingTest, f)))
        }
        crate::settings::TestSource::CommonWords => {
            v.extend([TestLanguage, TestWordListSize].map(|f| (Section::TypingTest, f)))
        }
        crate::settings::TestSource::PseudoWords => v.push((Section::TypingTest, TestLanguage)),
    }
    v
}

/// Moves `delta` steps through a list of choices, wrapping around.
fn cycle<T: PartialEq + Clone>(all: &[T], current: &T, delta: i32) -> T {
    let n = all.len() as i32;
    if n == 0 {
        return current.clone();
    }
    let i = all.iter().position(|v| v == current).unwrap_or(0) as i32;
    all[((i + delta) % n + n) as usize % n as usize].clone()
}

fn step_f64(value: f64, delta: i32, step: f64) -> f64 {
    ((value + delta as f64 * step) / step).round() * step
}

fn on_off(v: bool) -> String {
    if v { "[x] on".into() } else { "[ ] off".into() }
}

fn percent(v: f64) -> String {
    format!("{:.0}%", v * 100.0)
}

fn book_name(id: &str) -> String {
    let b = Book::get(id);
    format!("{} — {}", b.title, b.author)
}

impl Field {
    pub fn label(self) -> String {
        use Field::*;
        match self {
            LessonType => "Lesson type".into(),
            NaturalWords => "Prefer natural words".into(),
            KeyboardOrder => "Letters in keyboard order".into(),
            AlphabetSize => "Unlock more letters".into(),
            RecoverKeys => "Unlock a next key only when".into(),
            WordListSize => "Word list size".into(),
            LongWordsOnly => "Long words only".into(),
            Book => "Book".into(),
            ParagraphIndex => "Start at paragraph".into(),
            BookLettersOnly | CustomLettersOnly => "Remove punctuation".into(),
            BookLowercase | CustomLowercase => "Transform to lowercase".into(),
            CustomText => "Custom text".into(),
            CustomRandomize => "Shuffle words".into(),
            CodeSyntax => "Syntax".into(),
            CodeFlag(i) => format!("Include {}", FLAGS[i]),
            Benford => "Benford's law".into(),
            Capitals => "Add capital letters".into(),
            Punctuators => "Add punctuation".into(),
            RepeatWords => "Repeat each word".into(),
            LessonLength => "Add words to lessons".into(),
            TargetSpeed => "Target typing speed".into(),
            DailyGoal => "Daily goal".into(),
            StopOnError => "Stop cursor on error".into(),
            ForgiveErrors => "Forgive errors".into(),
            SpaceSkipsWords => "Space skips words".into(),
            CaretShape => "Cursor shape".into(),
            Whitespace => "Whitespace".into(),
            PlaySounds => "Play sounds".into(),
            SoundTheme => "Sound theme".into(),
            SoundVolume => "Volume".into(),
            Language => "Language".into(),
            Layout => "Layout".into(),
            Geometry => "Geometry".into(),
            Zones => "Finger zones".into(),
            Emulation => "Layout emulation".into(),
            SystemLayout => "Your system layout".into(),
            KeyboardColors => "Colored keys".into(),
            KeyboardPointers => "Highlight keys".into(),
            SpeedUnit => "Measure typing speed in".into(),
            Theme => "Theme".into(),
            ColorMode => "Terminal colors".into(),
            View => "Practice view".into(),
            TestDuration => "Test length".into(),
            TestSource => "Text".into(),
            TestLanguage => "Language".into(),
            TestWordListSize => "Word list size".into(),
            TestBook => "Book".into(),
            TestParagraph => "Start at paragraph".into(),
        }
    }
}

/// The number of lines a text takes when wrapped at word boundaries.
fn wrapped_height(text: &str, width: usize) -> u16 {
    let width = width.max(1);
    let mut lines = 1;
    let mut used = 0;
    for word in text.split_whitespace() {
        let n = word.chars().count();
        if used > 0 && used + 1 + n > width {
            lines += 1;
            used = 0;
        }
        used += if used > 0 { 1 + n } else { n };
    }
    lines
}

fn step_list_size(value: u32, delta: i32) -> u32 {
    let step = if value < 100 || (value == 100 && delta < 0) {
        10
    } else {
        50
    };
    (value as i32 + delta * step).clamp(10, 1000) as u32
}
