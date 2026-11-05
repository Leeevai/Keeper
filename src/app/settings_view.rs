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
