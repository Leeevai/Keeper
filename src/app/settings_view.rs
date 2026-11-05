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
