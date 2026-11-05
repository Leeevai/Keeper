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

    pub fn completed(&self) -> bool {
        self.input.completed()
    }

    pub fn stats(&self) -> Stats {
        make_stats(&self.input.steps())
    }

    /// Adds a new result and returns the notifications it causes.
    pub fn accept(
        &mut self,
        result: &TypingResult,
        unit: crate::settings::SpeedUnit,
    ) -> Vec<String> {
        let mut events = Vec::new();
        self.key_stats.append(result);
        self.summary.append(result);
        self.streaks.append(result);
        self.lesson_keys = self.lesson.update(&self.key_stats);
        for key in self.lesson_keys.included() {
            if self.included.insert(key.letter.cp) {
                events.push(format!("New key unlocked: {}", key.letter.label));
            }
        }
        if result.speed > self.top_speed {
            if self.summary.count >= 3 {
                events.push(format!(
                    "New top speed: {} (was {})",
                    unit.format(result.speed),
                    unit.format(self.top_speed)
                ));
            }
            self.top_speed = result.speed;
        }
        if result.score > self.top_score {
            if self.summary.count >= 3 {
                events.push(format!(
                    "New top score: {} (was {})",
                    crate::ui::thousands(result.score),
                    crate::ui::thousands(self.top_score)
                ));
            }
            self.top_score = result.score;
        }
        events
    }

    /// Generates the text of a new lesson.
    pub fn next_lesson(&mut self) {
        let text = self.lesson.generate(&self.lesson_keys, &mut self.rng);
        self.input = TextInput::new(text, self.input.settings);
    }

    /// Starts the same text over.
    pub fn restart(&mut self) {
        self.input.reset();
    }

    /// The code points of the lesson, used to dim the other keys.
    fn active_code_points(&self) -> HashSet<u32> {
        let mut set: HashSet<u32> = self.lesson_keys.included().map(|k| k.letter.cp).collect();
        set.insert(0x20);
        set
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, ctx: &RenderContext) {
        let view = ctx.settings.view;
        let indicators = if view == View::Bare {
            Vec::new()
        } else {
            self.indicators(ctx, area.width)
        };
        let text_width = area.width.saturating_sub(6).min(72);
        let chars = self.input.output();
        let text_view = TextView {
            chars: &chars,
            styles: ctx.styles,
            whitespace: ctx.settings.whitespace_style,
            caret: ctx.settings.caret_shape,
            rtl: self.lesson.language.is_rtl()
                && !matches!(self.lesson.lesson_type, crate::settings::LessonType::Code),
            typo: self.input.has_typo(),
        };
        let text_height = text_view.height(text_width).max(3);
        let (_, kb_tall) = KeyboardView::size(&self.lesson.keyboard, true);
        let (kb_width, kb_short) = KeyboardView::size(&self.lesson.keyboard, false);
        let [ind_area, _, rest] = Layout::vertical([
            Constraint::Length(indicators.len() as u16),
            Constraint::Length(if indicators.is_empty() { 0 } else { 1 }),
            Constraint::Fill(1),
        ])
        .areas(area);
        // The notification line, the text box and a gap above the keyboard.
        let fixed = 1 + text_height + 2 + 1;
        let keyboard_height = if view != View::Normal || kb_width > area.width {
            0
        } else if rest.height >= fixed + kb_tall {
            kb_tall
        } else if rest.height >= fixed + kb_short {
            kb_short
        } else {
            0
        };
        let group_height = (fixed + keyboard_height).min(rest.height);
        let group = Rect {
            y: rest.y + rest.height.saturating_sub(group_height) / 2,
            height: group_height,
            ..rest
        };
        let [toast_area, text_area, _, kb_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(text_height + 2),
            Constraint::Length(1),
            Constraint::Length(keyboard_height),
        ])
        .areas(group);

        let ind_area = Rect {
            x: ind_area.x + 2,
            width: ind_area.width.saturating_sub(4),
            ..ind_area
        };
        frame.render_widget(Paragraph::new(indicators).style(ctx.styles.base), ind_area);

        if let Some(toast) = ctx.toast {
            let line = Line::from(Span::styled(toast.to_string(), ctx.styles.title)).centered();
            frame.render_widget(Paragraph::new(line), toast_area);
        }

        let box_area = crate::ui::centered(text_area, text_width + 4, text_height + 2);
        // The practiced language may differ from the language of the
        // layout, such as British English on the US layout.
        let title = format!(
            " {} · {} · {} ",
            self.lesson.lesson_type.name(),
            self.lesson.language.name,
            self.lesson.layout().name
        );
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(ctx.styles.border)
            .title(Span::styled(title, ctx.styles.name));
        let inner = block.inner(box_area);
        frame.render_widget(block, box_area);
        let inner = Rect {
            x: inner.x + 1,
            width: inner.width.saturating_sub(2),
            ..inner
        };
        let text_rect = crate::ui::centered(inner, inner.width, text_view.height(inner.width));
        if let Some(pos) = text_view.render(text_rect, frame.buffer_mut())
            && ctx.settings.caret_shape == crate::settings::CaretShape::Line
            && ctx.focused
        {
            frame.set_cursor_position(pos);
        }

        if keyboard_height > 0 {
            let mut keyboard = KeyboardView::new(&self.lesson.keyboard, ctx.styles);
            keyboard.zone_colors = ctx.settings.keyboard_colors;
            if ctx.settings.keyboard_pointers
                && let Some(&next) = self.input.remaining().first()
            {
                keyboard.pointers = KeyboardView::pointers_for(&self.lesson.keyboard, next);
            }
            if let Some((key, ok, at)) = &self.last_key
                && ctx.now - at < 250.0
            {
                keyboard.flash = Some((key.clone(), *ok));
            }
            if matches!(self.lesson.lesson_type, crate::settings::LessonType::Guided) {
                keyboard.active = Some(self.active_code_points());
            }
            frame.render_widget(keyboard, kb_area);
        }
    }

    fn indicators(&self, ctx: &RenderContext, width: u16) -> Vec<Line<'static>> {
        let s = ctx.styles;
        let unit = ctx.settings.speed_unit;
        let label = |name: &str| Span::styled(format!("{name:<13}"), s.name);
        let mut lines = Vec::new();

        let mut metrics = vec![label("Metrics:")];
        if self.summary.count == 0 {
            metrics.push(Span::styled("No lessons yet, start typing.", s.name));
        } else {
            let m = &self.summary;
            metrics.extend(name_value(s, "Speed", unit.format(m.speed.last)));
            metrics.push(Span::styled(
                format!(" ({})", signed(m.speed.delta, unit.format(m.speed.delta))),
                s.delta(m.speed.delta),
            ));
            metrics.push(Span::raw("   "));
            metrics.extend(name_value(
                s,
                "Accuracy",
                format!("{:.1}%", m.accuracy.last * 100.0),
            ));
            let d = m.accuracy.delta * 100.0;
            metrics.push(Span::styled(
                format!(" ({}%)", signed(d, format!("{d:.1}"))),
                s.delta(d),
            ));
            metrics.push(Span::raw("   "));
            metrics.extend(name_value(s, "Score", crate::ui::thousands(m.score.last)));
            let d = m.score.delta;
            metrics.push(Span::styled(
                format!(" ({})", signed(d, crate::ui::thousands(d))),
                s.delta(d),
            ));
        }
        lines.push(Line::from(metrics));

        // All keys, colored by confidence; may take several lines.
        let mut row = vec![label("All keys:")];
        let mut used = 13usize;
        for key in self.lesson_keys.iter() {
            let label_width = unicode_width::UnicodeWidthStr::width(key.letter.label.as_str()) + 1;
            if used + label_width > width.saturating_sub(4) as usize {
                lines.push(Line::from(std::mem::take(&mut row)));
                row.push(Span::raw(" ".repeat(13)));
                used = 13;
            }
            let mut style = key_style(s, key.is_included, key.best_confidence.or(key.confidence));
            if key.is_focused {
                style =
                    style.add_modifier(Modifier::REVERSED | Modifier::BOLD | Modifier::UNDERLINED);
            }
            if key.is_forced {
                style = style.add_modifier(Modifier::ITALIC);
            }
            row.push(Span::styled(key.letter.label.clone(), style));
            row.push(Span::raw(" "));
            used += label_width;
        }
        lines.push(Line::from(row));

        let mut current = vec![label("Current key:")];
        match self.lesson_keys.focused() {
            Some(key) => {
                current.push(Span::styled(
                    format!("{} ", key.letter.label),
                    s.value.add_modifier(Modifier::UNDERLINED),
                ));
                match (
                    key.time_to_type,
                    key.best_time_to_type,
                    key.confidence,
                    key.best_confidence,
                ) {
                    (Some(t), Some(b), Some(c), Some(bc)) => {
                        current.push(Span::raw(" "));
                        current.extend(name_value(
                            s,
                            "Last speed",
                            format!("{} ({:.0}%)", unit.format(time_to_speed(t)), c * 100.0),
                        ));
                        current.push(Span::raw("  "));
                        current.extend(name_value(
                            s,
                            "Top speed",
                            format!("{} ({:.0}%)", unit.format(time_to_speed(b)), bc * 100.0),
                        ));
                        let target = Target::new(ctx.settings);
                        let rate = LearningRate::from(&key.samples, &target);
                        current.push(Span::raw("  "));
                        current.push(Span::styled("Learning rate: ", s.name));
                        match rate {
                            Some(lr) => {
                                let v = lr.learning_rate;
                                // keybr.com shows one to three faces at 0, 5 and 10 CPM per lesson.
                                let arrows = match v {
                                    v if v >= 10.0 => "▲▲▲",
                                    v if v >= 5.0 => "▲▲",
                                    v if v > 0.0 => "▲",
                                    v if v <= -10.0 => "▼▼▼",
                                    v if v <= -5.0 => "▼▼",
                                    v if v < 0.0 => "▼",
                                    _ => "",
                                };
                                current.push(Span::styled(
                                    format!("{}/lesson {arrows}", signed(v, unit.format(v))),
                                    s.delta(v),
                                ));
                                if let Some(n) = lr.remaining_lessons {
                                    current.push(Span::styled(
                                        format!("  ~{n} lessons to go"),
                                        s.name,
                                    ));
                                }
                            }
                            None => current.push(Span::styled("need more data", s.name)),
                        }
                    }
                    _ => current.push(Span::styled(
                        " Not calibrated, keep typing to measure this key.",
                        s.name,
                    )),
                }
            }
            None => current.push(Span::styled("All keys are unlocked.", s.name)),
        }
        lines.push(Line::from(current));

        if ctx.settings.view == View::Normal || ctx.settings.view == View::Compact {
            let mut streaks = vec![label("Streaks:")];
            let active: Vec<_> = self
                .streaks
                .streaks()
                .into_iter()
                .filter(|st| !st.results.is_empty())
                .collect();
            if active.is_empty() {
                streaks.push(Span::styled("No accuracy streaks.", s.name));
            }
            for (i, streak) in active.iter().enumerate() {
                if i > 0 {
                    streaks.push(Span::raw(" "));
                }
                let n = streak.results.len();
                let lessons = if n == 1 {
                    "One lesson".to_string()
                } else {
                    format!("{n} lessons")
                };
                streaks.push(Span::styled(format!("{lessons} with "), s.name));
                streaks.push(Span::styled(
                    format!("{:.0}%", streak.level * 100.0),
                    s.value,
                ));
                streaks.push(Span::styled(" accuracy.", s.name));
            }
            lines.push(Line::from(streaks));

            if ctx.daily_goal.goal > 0 {
                let goal: &DailyGoal = &ctx.daily_goal;
                let (full, empty) = bar(goal.value, 20);
                let done = if goal.value >= 1.0 { s.more } else { s.accent };
                lines.push(Line::from(vec![
                    label("Daily goal:"),
                    Span::styled(format!("{:.0}%", goal.value * 100.0), s.value),
                    Span::styled(format!("/{} minutes  ", goal.goal), s.name),
                    Span::styled(full, done),
                    Span::styled(empty, s.dim),
                ]));
            }
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::result::TextType;
    use crate::settings::SpeedUnit;

    fn type_all(p: &mut Practice, start: f64) -> f64 {
        let mut now = start;
        while !p.completed() {
            let next = p.input.remaining()[0];
            now += 150.0;
            p.handle(InputAction::Char(next), now);
        }
        now
    }

    #[test]
    fn typing_a_lesson_produces_a_valid_result() {
        let settings = Settings::default();
        let mut p = Practice::with_rng(&settings, &[], Lcg::new(1));
        assert!(!p.completed());
        type_all(&mut p, 0.0);
        let stats = p.stats();
        assert_eq!(stats.errors, 0);
        assert!((stats.speed - 400.0).abs() < 5.0, "{}", stats.speed);
        let result = TypingResult::from_stats("en-us", TextType::Generated, 0, &stats);
        assert!(result.validate());
        let events = p.accept(&result, SpeedUnit::Wpm);
        // 400 CPM is above the target, every key gets calibrated.
        assert!(events.is_empty() || events.iter().all(|e| e.starts_with("New key")));
        p.next_lesson();
        assert!(!p.completed());
    }

    #[test]
    fn fast_typing_unlocks_new_keys() {
        let settings = Settings::default();
        let mut results = Vec::new();
        let mut events = Vec::new();
        let mut p = Practice::with_rng(&settings, &results, Lcg::new(2));
        let mut now = 0.0;
        for i in 0..6 {
            now = type_all(&mut p, now + 1000.0);
            let result = TypingResult::from_stats("en-us", TextType::Generated, i, &p.stats());
            events.extend(p.accept(&result, SpeedUnit::Wpm));
            results.push(result);
            p.next_lesson();
        }
        assert!(
            events.iter().any(|e| e.starts_with("New key unlocked")),
            "{events:?}"
        );
        assert!(p.lesson_keys.included().count() > 6);
    }

    #[test]
    fn emulates_the_practiced_layout() {
        let settings = Settings {
            layout: "en-colemak".into(),
            ..Settings::default()
        };
        let mut p = Practice::with_rng(&settings, &[], Lcg::new(3));
        let expected = p.input.remaining()[0];
        // Find the QWERTY key at the position of the expected Colemak letter.
        let colemak = &p.lesson.keyboard;
        let key = colemak.combo(expected).unwrap().key.clone();
        let qwerty = Keyboard::new(crate::keyboard::Layout::get("en-us"), "ansi101", "standard");
        let typed = qwerty.characters(&key)[0]
            .as_ref()
            .unwrap()
            .code_point()
            .unwrap();
        assert_eq!(
            p.handle(InputAction::Char(typed), 100.0),
            Feedback::Succeeded
        );
    }

    #[test]
    fn restart_and_clear() {
        let mut p = Practice::with_rng(&Settings::default(), &[], Lcg::new(4));
        let first = p.input.remaining()[0];
        p.handle(InputAction::Char(first), 10.0);
        assert_eq!(p.input.pos(), 1);
        p.restart();
        assert_eq!(p.input.pos(), 0);
        assert_eq!(
            p.handle(InputAction::Char(u32::from('#')), 20.0),
            Feedback::Failed
        );
        assert_eq!(p.handle(InputAction::ClearChar, 30.0), Feedback::Succeeded);
        assert_eq!(p.handle(InputAction::ClearWord, 40.0), Feedback::Succeeded);
    }
}
