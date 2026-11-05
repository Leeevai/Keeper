//! The practice screen: adaptive lessons with live statistics.

use std::collections::HashSet;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use super::{InputAction, RenderContext};
use crate::keyboard::Keyboard;
use crate::lesson::{LearningRate, Lesson, LessonKeys, Target};
use crate::result::{
    DailyGoal, KeyStatsMap, StreakList, SummaryStats, TypingResult, time_to_speed,
};
use crate::rng::Lcg;
use crate::settings::{Emulation, Settings, View};
use crate::textinput::{Feedback, Stats, TextInput, make_stats};
use crate::ui::keyboard::{KeyboardView, key_style};
use crate::ui::text::TextView;
use crate::ui::{bar, name_value, signed};

pub struct Practice {
    pub lesson: Lesson,
    pub key_stats: KeyStatsMap,
    pub summary: SummaryStats,
    pub streaks: StreakList,
    pub lesson_keys: LessonKeys,
    pub input: TextInput,
    rng: Lcg,
    /// When the previous key was pressed, in milliseconds.
    last_input: f64,
    system_keyboard: Option<Keyboard>,
    /// The last pressed key, whether it was right, and when.
    pub last_key: Option<(String, bool, f64)>,
    included: HashSet<u32>,
    top_speed: f64,
    top_score: f64,
}

impl Practice {
    pub fn new(settings: &Settings, results: &[TypingResult]) -> Self {
        Self::with_rng(settings, results, Lcg::from_time())
    }

    pub fn with_rng(settings: &Settings, results: &[TypingResult], mut rng: Lcg) -> Self {
        let mut lesson = Lesson::new(settings);
        let filtered = lesson.filter(results);
        let key_stats = KeyStatsMap::from_results(lesson.letters(), filtered.iter().copied());
        let mut summary = SummaryStats::default();
        let mut streaks = StreakList::default();
        let (mut top_speed, mut top_score) = (0.0f64, 0.0f64);
        for result in &filtered {
            summary.append(result);
            streaks.append(result);
            top_speed = top_speed.max(result.speed);
            top_score = top_score.max(result.score);
        }
        let lesson_keys = lesson.update(&key_stats);
        let included = lesson_keys.included().map(|k| k.letter.cp).collect();
        let text = lesson.generate(&lesson_keys, &mut rng);
        let layout = lesson.layout();
        let emulate = settings.emulation == Emulation::Forward
            && layout.emulate
            && layout.id != settings.system_layout;
        let system_keyboard = emulate.then(|| {
            let system = crate::keyboard::Layout::get(&settings.system_layout);
            Keyboard::new(system, &system.geometries[0], "standard")
        });
        Self {
            input: TextInput::new(text, settings.text_input()),
            lesson,
            key_stats,
            summary,
            streaks,
            lesson_keys,
            rng,
            last_input: 0.0,
            system_keyboard,
            last_key: None,
            included,
            top_speed,
            top_score,
        }
    }

    /// Applies a key press at `now` milliseconds and returns the feedback.
    pub fn handle(&mut self, action: InputAction, now: f64) -> Feedback {
        match action {
            InputAction::Char(typed) => {
                let cp = match &self.system_keyboard {
                    Some(system) => self.lesson.keyboard.emulate(system, typed),
                    None => typed,
                };
                // Like keybr.com, every modifier key counts as a key press.
                let modifiers = self
                    .lesson
                    .keyboard
                    .combo(cp)
                    .map_or(0, |c| c.modifier_count());
                let time_to_type = (now - self.last_input) / (modifiers as f64 + 1.0);
                self.last_input = now;
                let feedback = self.input.append_char(now, cp, time_to_type);
                if let Some(combo) = self.lesson.keyboard.combo(cp) {
                    self.last_key = Some((combo.key.clone(), feedback != Feedback::Failed, now));
                }
                feedback
            }
            InputAction::LineBreak => {
                let time_to_type = now - self.last_input;
                self.last_input = now;
                self.input.append_line_break(now, time_to_type)
            }
            InputAction::ClearChar => self.input.clear_char(),
            InputAction::ClearWord => self.input.clear_word(),
        }
    }
}
