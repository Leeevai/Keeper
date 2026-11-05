//! All user settings, with the keys, defaults and ranges of keybr.com.

use serde::{Deserialize, Serialize};

use crate::keyboard::{Geometry, Keyboard, Layout};
use crate::language::Language;
use crate::result::TextType;
use crate::textinput::TextInputSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LessonType {
    /// Pseudo-words from the letters unlocked so far.
    #[default]
    Guided,
    /// The most common words of the language.
    Wordlist,
    Books,
    Custom,
    Code,
    Numbers,
}

impl LessonType {
    pub const ALL: [LessonType; 6] = [
        LessonType::Guided,
        LessonType::Wordlist,
        LessonType::Books,
        LessonType::Custom,
        LessonType::Code,
        LessonType::Numbers,
    ];

    pub fn name(self) -> &'static str {
        match self {
            LessonType::Guided => "Guided lessons",
            LessonType::Wordlist => "Common words",
            LessonType::Books => "Books",
            LessonType::Custom => "Custom text",
            LessonType::Code => "Source code",
            LessonType::Numbers => "Numbers",
        }
    }

    pub fn text_type(self) -> TextType {
        match self {
            LessonType::Guided => TextType::Generated,
            LessonType::Wordlist | LessonType::Books | LessonType::Custom => TextType::Natural,
            LessonType::Code => TextType::Code,
            LessonType::Numbers => TextType::Numbers,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SpeedUnit {
    #[default]
    Wpm,
    Wps,
    Cpm,
    Cps,
}

impl SpeedUnit {
    pub const ALL: [SpeedUnit; 4] = [
        SpeedUnit::Wpm,
        SpeedUnit::Wps,
        SpeedUnit::Cpm,
        SpeedUnit::Cps,
    ];

    pub fn id(self) -> &'static str {
        match self {
            SpeedUnit::Wpm => "wpm",
            SpeedUnit::Wps => "wps",
            SpeedUnit::Cpm => "cpm",
            SpeedUnit::Cps => "cps",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            SpeedUnit::Wpm => "Words per minute",
            SpeedUnit::Wps => "Words per second",
            SpeedUnit::Cpm => "Characters per minute",
            SpeedUnit::Cps => "Characters per second",
        }
    }

    /// Converts characters per minute into this unit.
    pub fn measure(self, cpm: f64) -> f64 {
        match self {
            SpeedUnit::Wpm => cpm / 5.0,
            SpeedUnit::Wps => cpm / 300.0,
            SpeedUnit::Cpm => cpm,
            SpeedUnit::Cps => cpm / 60.0,
        }
    }

    /// Formats a speed given in characters per minute, e.g. `42.5wpm`.
    pub fn format(self, cpm: f64) -> String {
        format!("{}{}", self.format_value(cpm), self.id())
    }

    pub fn format_value(self, cpm: f64) -> String {
        let v = self.measure(cpm);
        match self {
            SpeedUnit::Wpm | SpeedUnit::Cpm => format!("{v:.1}"),
            SpeedUnit::Wps | SpeedUnit::Cps => format!("{v:.2}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CaretShape {
    Block,
    Box,
    Line,
    #[default]
    Underline,
}

impl CaretShape {
    pub const ALL: [CaretShape; 4] = [
        CaretShape::Block,
        CaretShape::Box,
        CaretShape::Line,
        CaretShape::Underline,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CaretShape::Block => "Block cursor",
            CaretShape::Box => "Box cursor",
            CaretShape::Line => "Line cursor",
            CaretShape::Underline => "Underline cursor",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum WhitespaceStyle {
    Space,
    Bar,
    #[default]
    Bullet,
}

impl WhitespaceStyle {
    pub const ALL: [WhitespaceStyle; 3] = [
        WhitespaceStyle::Space,
        WhitespaceStyle::Bar,
        WhitespaceStyle::Bullet,
    ];

    pub fn name(self) -> &'static str {
        match self {
            WhitespaceStyle::Space => "No whitespace",
            WhitespaceStyle::Bar => "Bar whitespace",
            WhitespaceStyle::Bullet => "Bullet whitespace",
        }
    }

    pub fn symbol(self) -> char {
        match self {
            WhitespaceStyle::Space => ' ',
            WhitespaceStyle::Bar => '␣',
            WhitespaceStyle::Bullet => '·',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PlaySounds {
    #[default]
    None,
    ErrorsOnly,
    All,
    KeysOnly,
}

impl PlaySounds {
    pub const ALL: [PlaySounds; 4] = [
        PlaySounds::None,
        PlaySounds::ErrorsOnly,
        PlaySounds::All,
        PlaySounds::KeysOnly,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PlaySounds::None => "No sounds",
            PlaySounds::ErrorsOnly => "Error sounds only",
            PlaySounds::All => "All sounds",
            PlaySounds::KeysOnly => "Key sounds only",
        }
    }

    pub fn keys(self) -> bool {
        matches!(self, PlaySounds::All | PlaySounds::KeysOnly)
    }

    pub fn errors(self) -> bool {
        matches!(self, PlaySounds::All | PlaySounds::ErrorsOnly)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SoundTheme {
    #[default]
    Default,
    Mechanical1,
    Mechanical2,
    Typewriter1,
    Typewriter2,
}

impl SoundTheme {
    pub const ALL: [SoundTheme; 5] = [
        SoundTheme::Default,
        SoundTheme::Mechanical1,
        SoundTheme::Mechanical2,
        SoundTheme::Typewriter1,
        SoundTheme::Typewriter2,
    ];

    pub fn name(self) -> &'static str {
        match self {
            SoundTheme::Default => "Default",
            SoundTheme::Mechanical1 => "Mechanical 1",
            SoundTheme::Mechanical2 => "Mechanical 2",
            SoundTheme::Typewriter1 => "Typewriter 1",
            SoundTheme::Typewriter2 => "Typewriter 2",
        }
    }
}

/// How typed characters are mapped to the practiced layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Emulation {
    /// Characters are used as typed.
    None,
    /// Keys typed on the system layout are translated into the practiced layout.
    #[default]
    Forward,
    /// The keyboard hardware already emulates the layout.
    Reverse,
}

impl Emulation {
    pub const ALL: [Emulation; 3] = [Emulation::None, Emulation::Forward, Emulation::Reverse];

    pub fn name(self) -> &'static str {
        match self {
            Emulation::None => "Off",
            Emulation::Forward => "Emulate layout",
            Emulation::Reverse => "Keyboard hardware emulates layout",
        }
    }
}

/// How much of the practice screen is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum View {
    #[default]
    Normal,
    Compact,
    Bare,
}

impl View {
    pub fn next(self) -> View {
        match self {
            View::Normal => View::Compact,
            View::Compact => View::Bare,
            View::Bare => View::Normal,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            View::Normal => "Normal",
            View::Compact => "Compact",
            View::Bare => "Bare",
        }
    }
}

/// Terminal color support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode {
    #[default]
    Auto,
    Truecolor,
    #[serde(rename = "256")]
    Ansi256,
}

impl ColorMode {
    pub const ALL: [ColorMode; 3] = [ColorMode::Auto, ColorMode::Truecolor, ColorMode::Ansi256];

    pub fn name(self) -> &'static str {
        match self {
            ColorMode::Auto => "Detect",
            ColorMode::Truecolor => "24-bit color",
            ColorMode::Ansi256 => "256 colors",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TestSource {
    #[default]
    CommonWords,
    PseudoWords,
    Book,
}

impl TestSource {
    pub const ALL: [TestSource; 3] = [
        TestSource::CommonWords,
        TestSource::PseudoWords,
        TestSource::Book,
    ];

    pub fn name(self) -> &'static str {
        match self {
            TestSource::CommonWords => "Common words",
            TestSource::PseudoWords => "Pseudo words",
            TestSource::Book => "Book",
        }
    }
}

/// The length of a typing test, by time or by characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TestDuration {
    #[default]
    Seconds15,
    Seconds30,
    Seconds60,
    Chars100,
    Chars500,
    Chars1000,
}

impl TestDuration {
    pub const ALL: [TestDuration; 6] = [
        TestDuration::Seconds15,
        TestDuration::Seconds30,
        TestDuration::Seconds60,
        TestDuration::Chars100,
        TestDuration::Chars500,
        TestDuration::Chars1000,
    ];

    pub fn name(self) -> &'static str {
        match self {
            TestDuration::Seconds15 => "15 seconds",
            TestDuration::Seconds30 => "30 seconds",
            TestDuration::Seconds60 => "one minute",
            TestDuration::Chars100 => "100 characters",
            TestDuration::Chars500 => "500 characters",
            TestDuration::Chars1000 => "1000 characters",
        }
    }

    /// Milliseconds for timed tests.
    pub fn time(self) -> Option<f64> {
        match self {
            TestDuration::Seconds15 => Some(15_000.0),
            TestDuration::Seconds30 => Some(30_000.0),
            TestDuration::Seconds60 => Some(60_000.0),
            _ => None,
        }
    }

    /// Characters for tests of a fixed length.
    pub fn length(self) -> Option<usize> {
        match self {
            TestDuration::Chars100 => Some(100),
            TestDuration::Chars500 => Some(500),
            TestDuration::Chars1000 => Some(1000),
            _ => None,
        }
    }
}

macro_rules! defaults {
    ($($name:ident: $ty:ty = $value:expr;)*) => {
        $(fn $name() -> $ty { $value })*
    };
}

defaults! {
    d_true: bool = true;
    d_word_list_size: u32 = 1000;
    d_book: String = "en-alice-wonderland".into();
    d_custom_text: String = "The quick brown fox jumps over the lazy dog.".into();
    d_syntax: String = "html".into();
    d_repeat_words: u32 = 1;
    d_target_speed: u32 = 175;
    d_daily_goal: u32 = 30;
    d_sound_volume: f64 = 0.5;
    d_language: String = "en".into();
    d_layout: String = "en-us".into();
    d_geometry: String = "ansi101".into();
    d_zones: String = "standard".into();
    d_theme: String = "system".into();
}

/// The settings, serialized as a flat object with keybr.com's keys.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    // ----- Lessons -----
    #[serde(rename = "lesson.type", default)]
    pub lesson_type: LessonType,
    /// 0 to 1, adds up to 100 characters to the lessons.
    #[serde(rename = "lesson.length", default)]
    pub lesson_length: f64,
    #[serde(rename = "lesson.guided.naturalWords", default = "d_true")]
    pub natural_words: bool,
    #[serde(rename = "lesson.guided.keyboardOrder", default)]
    pub keyboard_order: bool,
    /// 0 to 1, manually unlocks letters.
    #[serde(rename = "lesson.guided.alphabetSize", default)]
    pub alphabet_size: f64,
    #[serde(rename = "lesson.guided.recoverKeys", default)]
    pub recover_keys: bool,
    #[serde(rename = "lesson.wordList.wordListSize", default = "d_word_list_size")]
    pub word_list_size: u32,
    #[serde(rename = "lesson.wordList.longWordsOnly", default)]
    pub long_words_only: bool,
    #[serde(rename = "lesson.books.book", default = "d_book")]
    pub book: String,
    #[serde(rename = "lesson.books.paragraphIndex", default)]
    pub paragraph_index: u32,
    #[serde(rename = "lesson.books.lettersOnly", default)]
    pub book_letters_only: bool,
    #[serde(rename = "lesson.books.lowercase", default)]
    pub book_lowercase: bool,
    #[serde(rename = "lesson.customText.content", default = "d_custom_text")]
    pub custom_text: String,
    #[serde(rename = "lesson.customText.lettersOnly", default = "d_true")]
    pub custom_letters_only: bool,
    #[serde(rename = "lesson.customText.lowercase", default = "d_true")]
    pub custom_lowercase: bool,
    #[serde(rename = "lesson.customText.randomize", default)]
    pub custom_randomize: bool,
    #[serde(rename = "lesson.numbers.benford", default = "d_true")]
    pub benford: bool,
    #[serde(rename = "lesson.code.syntax", default = "d_syntax")]
    pub code_syntax: String,
    #[serde(rename = "lesson.code.flags", default)]
    pub code_flags: Vec<String>,
    /// 0 to 1, the share of capitalized words.
    #[serde(rename = "lesson.capitals", default)]
    pub capitals: f64,
    /// 0 to 1, the share of words with punctuation.
    #[serde(rename = "lesson.punctuators", default)]
    pub punctuators: f64,
    #[serde(rename = "lesson.repeatWords", default = "d_repeat_words")]
    pub repeat_words: u32,
    /// Characters per minute.
    #[serde(rename = "lesson.targetSpeed", default = "d_target_speed")]
    pub target_speed: u32,
    /// Minutes per day, zero when not set.
    #[serde(rename = "lesson.dailyGoal", default = "d_daily_goal")]
    pub daily_goal: u32,

    // ----- Typing -----
    #[serde(rename = "textInput.stopOnError", default = "d_true")]
    pub stop_on_error: bool,
    #[serde(rename = "textInput.forgiveErrors", default = "d_true")]
    pub forgive_errors: bool,
    #[serde(rename = "textInput.spaceSkipsWords", default)]
    pub space_skips_words: bool,
    #[serde(rename = "textDisplay.caretShapeStyle", default)]
    pub caret_shape: CaretShape,
    #[serde(rename = "textDisplay.whitespaceStyle", default)]
    pub whitespace_style: WhitespaceStyle,
    #[serde(rename = "textInput.playSounds", default)]
    pub play_sounds: PlaySounds,
    #[serde(rename = "textInput.soundVolume", default = "d_sound_volume")]
    pub sound_volume: f64,
    #[serde(rename = "textInput.soundTheme", default)]
    pub sound_theme: SoundTheme,

    // ----- Keyboard -----
    #[serde(rename = "keyboard.language", default = "d_language")]
    pub language: String,
    #[serde(rename = "keyboard.layout", default = "d_layout")]
    pub layout: String,
    #[serde(rename = "keyboard.geometry", default = "d_geometry")]
    pub geometry: String,
    #[serde(rename = "keyboard.zones", default = "d_zones")]
    pub zones: String,
    #[serde(rename = "keyboard.emulation", default)]
    pub emulation: Emulation,
    /// Color the keyboard by finger zones.
    #[serde(rename = "keyboard.colors", default = "d_true")]
    pub keyboard_colors: bool,
    /// Highlight the key to press next.
    #[serde(rename = "keyboard.pointers", default = "d_true")]
    pub keyboard_pointers: bool,
    /// The layout configured in the operating system, used for emulation.
    #[serde(rename = "keyboard.systemLayout", default = "d_layout")]
    pub system_layout: String,
    #[serde(rename = "prefs.practice.view", default)]
    pub view: View,

    // ----- Miscellaneous -----
    #[serde(rename = "ui.speedUnit", default)]
    pub speed_unit: SpeedUnit,
    #[serde(rename = "ui.theme", default = "d_theme")]
    pub theme: String,
    #[serde(rename = "ui.colorMode", default)]
    pub color_mode: ColorMode,

    // ----- Typing test -----
    #[serde(rename = "typingTest.textSource.type", default)]
    pub test_source: TestSource,
    #[serde(rename = "typingTest.textSource.language", default = "d_language")]
    pub test_language: String,
    #[serde(
        rename = "typingTest.wordList.wordListSize",
        default = "d_word_list_size"
    )]
    pub test_word_list_size: u32,
    #[serde(rename = "typingTest.book", default = "d_book")]
    pub test_book: String,
    #[serde(rename = "typingTest.book.paragraphIndex", default)]
    pub test_paragraph_index: u32,
    #[serde(rename = "typingTest.duration", default)]
    pub test_duration: TestDuration,
}

impl Default for Settings {
    fn default() -> Self {
        serde_json::from_str("{}").expect("defaults")
    }
}

/// The resolved keyboard configuration.
#[derive(Debug, Clone)]
pub struct KeyboardOptions {
    pub language: &'static Language,
    pub layout: &'static Layout,
    pub geometry: &'static Geometry,
    pub zones: String,
}

impl Settings {
    /// Clamps numbers into their ranges and replaces unknown ids.
    pub fn normalize(mut self) -> Self {
        let clamp01 = |v: f64| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        self.lesson_length = clamp01(self.lesson_length);
        self.alphabet_size = clamp01(self.alphabet_size);
        self.capitals = clamp01(self.capitals);
        self.punctuators = clamp01(self.punctuators);
        self.sound_volume = clamp01(self.sound_volume);
        self.word_list_size = self.word_list_size.clamp(10, 1000);
        self.test_word_list_size = self.test_word_list_size.clamp(10, 1000);
        self.paragraph_index = self.paragraph_index.min(1000);
        self.test_paragraph_index = self.test_paragraph_index.min(1000);
        self.repeat_words = self.repeat_words.clamp(1, 10);
        self.target_speed = self.target_speed.clamp(75, 750);
        self.daily_goal = self.daily_goal.min(120);
        if self.custom_text.chars().count() > 10_000 {
            self.custom_text = self.custom_text.chars().take(10_000).collect();
        }
        if crate::content::Book::find(&self.book).is_none() {
            self.book = d_book();
        }
        if crate::content::Book::find(&self.test_book).is_none() {
            self.test_book = d_book();
        }
        if Language::find(&self.test_language).is_none() {
            self.test_language = d_language();
        }
        if Layout::find(&self.system_layout).is_none() {
            self.system_layout = d_layout();
        }
        if crate::theme::Theme::find(&self.theme).is_none() {
            self.theme = d_theme();
        }
        self.code_flags
            .retain(|f| crate::code::FLAGS.contains(&f.as_str()));
        let options = self.keyboard_options();
        self.language = options.language.id.clone();
        self.layout = options.layout.id.clone();
        self.geometry = options.geometry.id.clone();
        self.zones = options.zones;
        self
    }

    /// Resolves the language, layout, geometry and zones the same way as
    /// keybr.com: each choice must be compatible with the previous ones.
    pub fn keyboard_options(&self) -> KeyboardOptions {
        let language = Language::get(&self.language);
        let mut layout = Layout::select(language);
        if let Some(l) = Layout::find(&self.layout)
            && l.language().script == language.script
        {
            layout = l;
        }
        let geometry_id = if layout.geometries.contains(&self.geometry) {
            self.geometry.clone()
        } else {
            layout.geometries[0].clone()
        };
        let geometry = Geometry::get(&geometry_id);
        let zones = if layout.layout_mod.is_none() && geometry.zones.contains(&self.zones) {
            self.zones.clone()
        } else {
            geometry.zones.first().cloned().unwrap_or_else(d_zones)
        };
        KeyboardOptions {
            language,
            layout,
            geometry,
            zones,
        }
    }

    /// Selects a language with its default layout.
    pub fn set_language(&mut self, id: &str) {
        let language = Language::get(id);
        let layout = Layout::select(language);
        self.language = language.id.clone();
        self.layout = layout.id.clone();
        self.geometry = layout.geometries[0].clone();
        self.zones = d_zones();
    }

    /// Selects a layout and its first geometry.
    pub fn set_layout(&mut self, id: &str) {
        let layout = Layout::get(id);
        self.layout = layout.id.clone();
        self.geometry = layout.geometries[0].clone();
        self.zones = d_zones();
    }

    pub fn keyboard(&self) -> Keyboard {
        let o = self.keyboard_options();
        Keyboard::new(o.layout, &o.geometry.id, &o.zones)
    }

    pub fn text_input(&self) -> TextInputSettings {
        TextInputSettings {
            stop_on_error: self.stop_on_error,
            forgive_errors: self.forgive_errors,
            space_skips_words: self.space_skips_words,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_keybr() {
        let s = Settings::default();
        assert_eq!(s.lesson_type, LessonType::Guided);
        assert_eq!(s.target_speed, 175);
        assert_eq!(s.daily_goal, 30);
        assert!(s.natural_words && !s.recover_keys && !s.keyboard_order);
        assert!(s.stop_on_error && s.forgive_errors && !s.space_skips_words);
        assert_eq!(s.caret_shape, CaretShape::Underline);
        assert_eq!(s.whitespace_style, WhitespaceStyle::Bullet);
        assert_eq!(s.speed_unit, SpeedUnit::Wpm);
        assert_eq!(
            (s.language.as_str(), s.layout.as_str(), s.geometry.as_str()),
            ("en", "en-us", "ansi101")
        );
        assert_eq!(s.emulation, Emulation::Forward);
        assert_eq!(s.repeat_words, 1);
        assert!(s.custom_letters_only && s.custom_lowercase);
        assert_eq!(s.normalize(), Settings::default());
    }

    #[test]
    fn serializes_with_keybr_keys() {
        let s = Settings {
            target_speed: 250,
            caret_shape: CaretShape::Block,
            ..Default::default()
        };
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["lesson.targetSpeed"], 250);
        assert_eq!(json["textDisplay.caretShapeStyle"], "block");
        assert_eq!(json["lesson.type"], "guided");
        let back: Settings = serde_json::from_value(json).unwrap();
        assert_eq!(back, s);
        // Unknown and missing keys are fine.
        let partial: Settings =
            serde_json::from_str(r#"{"lesson.targetSpeed": 300, "x": 1}"#).unwrap();
        assert_eq!(partial.target_speed, 300);
        assert_eq!(partial.daily_goal, 30);
    }

    #[test]
    fn normalizes_out_of_range_values() {
        let s: Settings = serde_json::from_str(
            r#"{"lesson.targetSpeed": 5000, "lesson.capitals": 3, "lesson.repeatWords": 0,
                "keyboard.layout": "nope", "lesson.books.book": "nope", "ui.theme": "nope"}"#,
        )
        .unwrap();
        let s = s.normalize();
        assert_eq!(s.target_speed, 750);
        assert_eq!(s.capitals, 1.0);
        assert_eq!(s.repeat_words, 1);
        assert_eq!(s.layout, "en-us");
        assert_eq!(s.book, "en-alice-wonderland");
        assert_eq!(s.theme, "system");
    }

    #[test]
    fn keyboard_options_stay_compatible() {
        let mut s = Settings::default();
        s.set_language("de");
        assert_eq!(s.keyboard_options().layout.id, "de-de");
        // A Latin layout of another language is allowed.
        s.layout = "en-colemak".into();
        assert_eq!(s.keyboard_options().layout.id, "en-colemak");
        // A Cyrillic layout is not.
        s.layout = "ru-ru".into();
        assert_eq!(s.keyboard_options().layout.id, "de-de");
        // Unsupported geometries fall back to the first one.
        s.layout = "en-colemak-dh-matrix".into();
        s.geometry = "ansi101".into();
        assert_eq!(s.keyboard_options().geometry.id, "matrix");
        // Zones cannot be changed for layouts with their own zones.
        s.layout = "en-colemak-dh".into();
        s.zones = "symmetric".into();
        assert_eq!(s.keyboard_options().zones, "standard");
    }

    #[test]
    fn speed_units() {
        assert_eq!(SpeedUnit::Wpm.format(250.0), "50.0wpm");
        assert_eq!(SpeedUnit::Cps.format(300.0), "5.00cps");
        assert_eq!(SpeedUnit::Wps.format(300.0), "1.00wps");
        assert_eq!(SpeedUnit::Cpm.format(123.45), "123.5cpm");
    }
}
