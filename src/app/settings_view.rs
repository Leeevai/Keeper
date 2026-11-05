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

    pub fn value(self, s: &Settings) -> String {
        use Field::*;
        let options = s.keyboard_options();
        match self {
            LessonType => s.lesson_type.name().into(),
            NaturalWords => on_off(s.natural_words),
            KeyboardOrder => on_off(s.keyboard_order),
            AlphabetSize => {
                let keyboard = s.keyboard();
                let cps = keyboard.code_points();
                let letters = shared_model(options.language)
                    .letters
                    .iter()
                    .filter(|l| cps.has(l.cp))
                    .count();
                let extra = ((letters.saturating_sub(6)) as f64 * s.alphabet_size).round();
                format!(
                    "{} ({} letters)",
                    percent(s.alphabet_size),
                    6 + extra as usize
                )
            }
            RecoverKeys => {
                if s.recover_keys {
                    "all keys are fast".into()
                } else {
                    "the focused key is fast".into()
                }
            }
            WordListSize => format!("{} words", s.word_list_size),
            LongWordsOnly => on_off(s.long_words_only),
            Book => book_name(&s.book),
            ParagraphIndex => format!("{}", s.paragraph_index + 1),
            BookLettersOnly => on_off(s.book_letters_only),
            BookLowercase => on_off(s.book_lowercase),
            CustomText => {
                let preview: String = s.custom_text.chars().take(28).collect();
                let more = if s.custom_text.chars().count() > 28 {
                    "…"
                } else {
                    ""
                };
                format!("“{}{more}” (Enter to edit)", preview.replace('\n', " "))
            }
            CustomLettersOnly => on_off(s.custom_letters_only),
            CustomLowercase => on_off(s.custom_lowercase),
            CustomRandomize => on_off(s.custom_randomize),
            CodeSyntax => Syntax::get(&s.code_syntax).name.clone(),
            CodeFlag(i) => on_off(s.code_flags.iter().any(|f| f == FLAGS[i])),
            Benford => on_off(s.benford),
            Capitals => percent(s.capitals),
            Punctuators => percent(s.punctuators),
            RepeatWords => format!("{}×", s.repeat_words),
            LessonLength => format!("+{} characters", (s.lesson_length * 100.0).round()),
            TargetSpeed => s.speed_unit.format(s.target_speed as f64),
            DailyGoal => {
                if s.daily_goal == 0 {
                    "Not set".into()
                } else {
                    format!("{} minutes", s.daily_goal)
                }
            }
            StopOnError => on_off(s.stop_on_error),
            ForgiveErrors => on_off(s.forgive_errors),
            SpaceSkipsWords => on_off(s.space_skips_words),
            CaretShape => s.caret_shape.name().into(),
            Whitespace => s.whitespace_style.name().into(),
            PlaySounds => s.play_sounds.name().into(),
            SoundTheme => s.sound_theme.name().into(),
            SoundVolume => percent(s.sound_volume),
            Language => options.language.name.clone(),
            Layout => options.layout.name.clone(),
            Geometry => options.geometry.name.clone(),
            Zones => {
                if options.layout.layout_mod.is_some() {
                    "set by the layout".into()
                } else {
                    zone_mods()
                        .iter()
                        .find(|(id, _)| *id == options.zones)
                        .map_or(options.zones.clone(), |(_, n)| n.to_string())
                }
            }
            Emulation => {
                if options.layout.emulate {
                    s.emulation.name().into()
                } else {
                    format!("{} (not needed for this layout)", s.emulation.name())
                }
            }
            SystemLayout => KeyboardLayout::get(&s.system_layout).full_name(),
            KeyboardColors => on_off(s.keyboard_colors),
            KeyboardPointers => on_off(s.keyboard_pointers),
            SpeedUnit => s.speed_unit.name().into(),
            Theme => crate::theme::Theme::get(&s.theme).name.into(),
            ColorMode => s.color_mode.name().into(),
            View => s.view.name().into(),
            TestDuration => s.test_duration.name().into(),
            TestSource => s.test_source.name().into(),
            TestLanguage => crate::language::Language::get(&s.test_language)
                .name
                .clone(),
            TestWordListSize => format!("{} words", s.test_word_list_size),
            TestBook => book_name(&s.test_book),
            TestParagraph => format!("{}", s.test_paragraph_index + 1),
        }
    }

    /// Changes the value by `delta` steps.
    pub fn change(self, s: &mut Settings, delta: i32) {
        use Field::*;
        let options = s.keyboard_options();
        match self {
            LessonType => {
                s.lesson_type = cycle(&crate::settings::LessonType::ALL, &s.lesson_type, delta)
            }
            NaturalWords => s.natural_words = !s.natural_words,
            KeyboardOrder => s.keyboard_order = !s.keyboard_order,
            AlphabetSize => {
                s.alphabet_size = step_f64(s.alphabet_size, delta, 0.05).clamp(0.0, 1.0)
            }
            RecoverKeys => s.recover_keys = !s.recover_keys,
            WordListSize => s.word_list_size = step_list_size(s.word_list_size, delta),
            LongWordsOnly => s.long_words_only = !s.long_words_only,
            Book => {
                let ids: Vec<String> = crate::content::Book::all()
                    .iter()
                    .map(|b| b.id.clone())
                    .collect();
                s.book = cycle(&ids, &s.book, delta);
                s.paragraph_index = 0;
            }
            ParagraphIndex => {
                let n = crate::content::Book::get(&s.book).paragraphs().len() as i64;
                s.paragraph_index =
                    (s.paragraph_index as i64 + delta as i64).rem_euclid(n.clamp(1, 1001)) as u32;
            }
            BookLettersOnly => s.book_letters_only = !s.book_letters_only,
            BookLowercase => s.book_lowercase = !s.book_lowercase,
            CustomText => {}
            CustomLettersOnly => s.custom_letters_only = !s.custom_letters_only,
            CustomLowercase => s.custom_lowercase = !s.custom_lowercase,
            CustomRandomize => s.custom_randomize = !s.custom_randomize,
            CodeSyntax => {
                let ids: Vec<String> = Syntax::all().iter().map(|x| x.id.clone()).collect();
                s.code_syntax = cycle(&ids, &s.code_syntax, delta);
            }
            CodeFlag(i) => {
                let flag = FLAGS[i].to_string();
                if let Some(pos) = s.code_flags.iter().position(|f| *f == flag) {
                    s.code_flags.remove(pos);
                } else {
                    s.code_flags.push(flag);
                }
            }
            Benford => s.benford = !s.benford,
            Capitals => s.capitals = step_f64(s.capitals, delta, 0.05).clamp(0.0, 1.0),
            Punctuators => s.punctuators = step_f64(s.punctuators, delta, 0.05).clamp(0.0, 1.0),
            RepeatWords => s.repeat_words = (s.repeat_words as i32 + delta).clamp(1, 10) as u32,
            LessonLength => {
                s.lesson_length = step_f64(s.lesson_length, delta, 0.05).clamp(0.0, 1.0)
            }
            TargetSpeed => {
                s.target_speed = (s.target_speed as i32 + delta * 5).clamp(75, 750) as u32
            }
            DailyGoal => s.daily_goal = (s.daily_goal as i32 + delta * 5).clamp(0, 120) as u32,
            StopOnError => s.stop_on_error = !s.stop_on_error,
            ForgiveErrors => s.forgive_errors = !s.forgive_errors,
            SpaceSkipsWords => s.space_skips_words = !s.space_skips_words,
            CaretShape => {
                s.caret_shape = cycle(&crate::settings::CaretShape::ALL, &s.caret_shape, delta)
            }
            Whitespace => {
                s.whitespace_style = cycle(&WhitespaceStyle::ALL, &s.whitespace_style, delta)
            }
            PlaySounds => {
                s.play_sounds = cycle(&crate::settings::PlaySounds::ALL, &s.play_sounds, delta)
            }
            SoundTheme => {
                s.sound_theme = cycle(&crate::settings::SoundTheme::ALL, &s.sound_theme, delta)
            }
            SoundVolume => s.sound_volume = step_f64(s.sound_volume, delta, 0.1).clamp(0.0, 1.0),
            Language => {
                let ids: Vec<String> = crate::language::Language::all()
                    .iter()
                    .map(|l| l.id.clone())
                    .collect();
                let id = cycle(&ids, &options.language.id, delta);
                s.set_language(&id);
            }
            Layout => {
                let ids: Vec<String> = KeyboardLayout::selectable(options.language)
                    .iter()
                    .map(|l| l.id.clone())
                    .collect();
                let id = cycle(&ids, &options.layout.id, delta);
                s.set_layout(&id);
            }
            Geometry => {
                s.geometry = cycle(&options.layout.geometries, &options.geometry.id, delta);
                s.zones = "standard".into();
            }
            Zones => {
                if options.layout.layout_mod.is_none() {
                    s.zones = cycle(&options.geometry.zones, &options.zones, delta);
                }
            }
            Emulation => s.emulation = cycle(&crate::settings::Emulation::ALL, &s.emulation, delta),
            SystemLayout => {
                let ids: Vec<String> = KeyboardLayout::all()
                    .iter()
                    .filter(|l| l.language().script == options.language.script)
                    .map(|l| l.id.clone())
                    .collect();
                s.system_layout = cycle(&ids, &s.system_layout, delta);
            }
            KeyboardColors => s.keyboard_colors = !s.keyboard_colors,
            KeyboardPointers => s.keyboard_pointers = !s.keyboard_pointers,
            SpeedUnit => {
                s.speed_unit = cycle(&crate::settings::SpeedUnit::ALL, &s.speed_unit, delta)
            }
            Theme => {
                let ids: Vec<String> = crate::theme::Theme::all()
                    .iter()
                    .map(|t| t.id.to_string())
                    .collect();
                s.theme = cycle(&ids, &s.theme, delta);
            }
            ColorMode => {
                s.color_mode = cycle(&crate::settings::ColorMode::ALL, &s.color_mode, delta)
            }
            View => {
                let all = [
                    crate::settings::View::Normal,
                    crate::settings::View::Compact,
                    crate::settings::View::Bare,
                ];
                s.view = cycle(&all, &s.view, delta);
            }
            TestDuration => {
                s.test_duration =
                    cycle(&crate::settings::TestDuration::ALL, &s.test_duration, delta)
            }
            TestSource => {
                s.test_source = cycle(&crate::settings::TestSource::ALL, &s.test_source, delta)
            }
            TestLanguage => {
                let ids: Vec<String> = crate::language::Language::all()
                    .iter()
                    .map(|l| l.id.clone())
                    .collect();
                s.test_language = cycle(&ids, &s.test_language, delta);
            }
            TestWordListSize => {
                s.test_word_list_size = step_list_size(s.test_word_list_size, delta)
            }
            TestBook => {
                let ids: Vec<String> = crate::content::Book::all()
                    .iter()
                    .map(|b| b.id.clone())
                    .collect();
                s.test_book = cycle(&ids, &s.test_book, delta);
                s.test_paragraph_index = 0;
            }
            TestParagraph => {
                let n = crate::content::Book::get(&s.test_book).paragraphs().len() as i64;
                s.test_paragraph_index = (s.test_paragraph_index as i64 + delta as i64)
                    .rem_euclid(n.clamp(1, 1001)) as u32;
            }
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
