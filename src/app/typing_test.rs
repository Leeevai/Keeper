//! The typing test screen, ported from keybr's typing test page.

use std::sync::Arc;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Sparkline};

use super::{InputAction, RenderContext};
use crate::content::{Book, word_list};
use crate::language::Language;
use crate::lesson::shared_model;
use crate::phonetic::{Filter, PhoneticModel};
use crate::rng::{Lcg, random_sample};
use crate::settings::{Settings, TestDuration, TestSource};
use crate::stats::Distribution;
use crate::storage::TestResult;
use crate::textinput::{Char, Feedback, Step, TextInput, compute_speed, make_stats, plain};
use crate::ui::text::TextView;
use crate::ui::{format_duration, thousands};
use unicode_width::UnicodeWidthStr;

const NUM_LINES: usize = 5;
const NUM_COLS: usize = 55;

enum Source {
    Words(Vec<String>),
    Pseudo(Arc<PhoneticModel>),
    Book { words: Vec<String>, index: usize },
}

impl Source {
    fn new(settings: &Settings) -> Self {
        match settings.test_source {
            TestSource::CommonWords => {
                let words: Vec<String> = word_list(&settings.test_language)
                    .iter()
                    .take(settings.test_word_list_size as usize)
                    .cloned()
                    .collect();
                if words.is_empty() {
                    Source::Words(vec!["?".into()])
                } else {
                    Source::Words(words)
                }
            }
            TestSource::PseudoWords => {
                Source::Pseudo(shared_model(Language::get(&settings.test_language)))
            }
            TestSource::Book => {
                let paragraphs = Book::get(&settings.test_book).paragraphs();
                let start = (settings.test_paragraph_index as usize).min(paragraphs.len());
                let words = paragraphs[start..]
                    .iter()
                    .chain(&paragraphs[..start])
                    .flat_map(|p| p.split_whitespace().map(String::from).collect::<Vec<_>>())
                    .collect();
                Source::Book { words, index: 0 }
            }
        }
    }

    fn next_word(&mut self, rng: &mut Lcg) -> String {
        match self {
            Source::Words(words) => random_sample(words, rng).clone(),
            Source::Pseudo(model) => {
                let w = model.next_word(&Filter::default(), rng);
                if w.is_empty() { "?".into() } else { w }
            }
            Source::Book { words, index } => {
                if words.is_empty() {
                    return "?".into();
                }
                if *index >= words.len() {
                    *index = 0;
                }
                *index += 1;
                words[*index - 1].clone()
            }
        }
    }
}

/// The outcome of a finished test.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub result: TestResult,
    /// Speed over the test, in characters per minute per two seconds.
    pub speeds: Vec<f64>,
}

pub struct TypingTest {
    source: Source,
    rng: Lcg,
    duration: TestDuration,
    source_id: String,
    lines: Vec<String>,
    /// What was typed on the lines above the active one.
    typed: Vec<Option<Vec<Char>>>,
    active: usize,
    input: TextInput,
    steps: Vec<Step>,
    last_input: f64,
    pub report: Option<Report>,
}

impl TypingTest {
    pub fn new(settings: &Settings) -> Self {
        Self::with_rng(settings, Lcg::from_time())
    }

    pub fn with_rng(settings: &Settings, mut rng: Lcg) -> Self {
        let mut source = Source::new(settings);
        let lines: Vec<String> = (0..NUM_LINES)
            .map(|_| generate_line(&mut source, &mut rng))
            .collect();
        let input = TextInput::new(plain(lines[0].clone()), settings.text_input());
        Self {
            source,
            rng,
            duration: settings.test_duration,
            source_id: id_of(settings.test_source),
            typed: vec![None; NUM_LINES],
            lines,
            active: 0,
            input,
            steps: Vec::new(),
            last_input: 0.0,
            report: None,
        }
    }

    /// The next character to type.
    pub fn next_char(&self) -> Option<char> {
        self.input
            .remaining()
            .first()
            .and_then(|&cp| char::from_u32(cp))
    }

    pub fn started(&self) -> bool {
        !self.steps.is_empty() || self.input.pos() > 0
    }

    fn all_steps(&self) -> Vec<Step> {
        let mut steps = self.steps.clone();
        steps.extend(self.input.steps());
        steps
    }

    /// Elapsed milliseconds since the first key press.
    fn elapsed(&self, now: f64) -> f64 {
        self.all_steps().first().map_or(0.0, |s| now - s.time_stamp)
    }

    /// Progress from 0 to 1.
    fn progress(&self, now: f64) -> f64 {
        match (self.duration.time(), self.duration.length()) {
            (Some(ms), _) => self.elapsed(now) / ms,
            (_, Some(chars)) => self.all_steps().len() as f64 / chars as f64,
            _ => 0.0,
        }
    }

    pub fn handle(&mut self, action: InputAction, now: f64) -> Option<Feedback> {
        if self.report.is_some() {
            return None;
        }
        // A key pressed after the time is up, before the timer noticed, ends
        // the test without counting.
        if self.duration.time().is_some() && self.started() && self.progress(now) >= 1.0 {
            self.finish();
            return None;
        }
        let feedback = match action {
            InputAction::Char(cp) => {
                let ttt = now - self.last_input;
                self.last_input = now;
                self.input.append_char(now, cp, ttt)
            }
            InputAction::LineBreak => {
                let ttt = now - self.last_input;
                self.last_input = now;
                self.input.append_line_break(now, ttt)
            }
            InputAction::ClearChar => self.input.clear_char(),
            InputAction::ClearWord => self.input.clear_word(),
        };
        if self.input.completed() {
            self.next_line();
        }
        if self.progress(now) >= 1.0 {
            self.finish();
        }
        Some(feedback)
    }

    /// Ends a timed test when its time is up.
    pub fn tick(&mut self, now: f64) -> bool {
        if self.report.is_none()
            && self.started()
            && self.duration.time().is_some()
            && self.progress(now) >= 1.0
        {
            self.finish();
            return true;
        }
        false
    }

    fn next_line(&mut self) {
        self.steps.extend(self.input.steps());
        self.typed[self.active] = Some(self.input.output());
        if self.active < NUM_LINES - 3 {
            self.active += 1;
        } else {
            self.lines.remove(0);
            self.typed.remove(0);
            self.lines
                .push(generate_line(&mut self.source, &mut self.rng));
            self.typed.push(None);
        }
        self.input = TextInput::new(plain(self.lines[self.active].clone()), self.input.settings);
    }

    fn finish(&mut self) {
        let steps = self.all_steps();
        let stats = make_stats(&steps);
        let speeds = speed_series(&steps);
        // A timed test lasts its whole duration, even if typing stopped early.
        let (time, speed) = match self.duration.time() {
            Some(ms) => (ms as u32, compute_speed(steps.len(), ms)),
            None => (stats.time, stats.speed),
        };
        self.report = Some(Report {
            result: TestResult {
                time_stamp: chrono::Utc::now().timestamp_millis(),
                speed,
                accuracy: stats.accuracy,
                length: stats.length,
                time,
                errors: stats.errors,
                duration: id_of(self.duration),
                source: self.source_id.clone(),
            },
            speeds,
        });
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        ctx: &RenderContext,
        history: &[TestResult],
    ) {
        let s = ctx.styles;
        let settings = ctx.settings;
        let width = (NUM_COLS as u16 + 6).min(area.width);
        let source = match settings.test_source {
            TestSource::Book => Book::get(&settings.test_book).title.clone(),
            TestSource::CommonWords => format!(
                "{} in {}",
                if settings.test_word_list_size >= 1000 {
                    "common words".to_string()
                } else {
                    format!("top {} words", settings.test_word_list_size)
                },
                Language::get(&settings.test_language).name
            ),
            TestSource::PseudoWords => format!(
                "pseudo words in {}",
                Language::get(&settings.test_language).name
            ),
        };
        let toolbar = Line::from(vec![
            Span::styled("Test: ", s.name),
            Span::styled(settings.test_duration.name(), s.value),
            Span::styled(" (Ctrl+D)  ", s.dim),
            Span::styled("Text: ", s.name),
            Span::styled(source, s.value),
            Span::styled(" (Ctrl+T)", s.dim),
        ])
        .centered();
        let [bar, _, status, body] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(area);
        frame.render_widget(Paragraph::new(toolbar), bar);

        if let Some(report) = &self.report {
            self.render_report(frame, body, ctx, report, history);
            return;
        }

        let now = ctx.now;
        let steps = self.all_steps();
        let status_line = if steps.is_empty() {
            Line::from(Span::styled("Start typing to begin the test.", s.accent))
        } else {
            let elapsed = self.elapsed(now);
            let speed = compute_speed(steps.len(), elapsed);
            let left = match (self.duration.time(), self.duration.length()) {
                (Some(ms), _) => format!("{:.0}s left", ((ms - elapsed) / 1000.0).max(0.0).ceil()),
                (_, Some(chars)) => format!("{}/{} characters", steps.len(), chars),
                _ => String::new(),
            };
            let (full, empty) = crate::ui::bar(self.progress(now), 30);
            Line::from(vec![
                Span::styled(format!("{left}  "), s.value),
                Span::styled(full, s.accent),
                Span::styled(empty, s.dim),
                Span::styled(format!("  {}", settings.speed_unit.format(speed)), s.value),
            ])
        };
        frame.render_widget(Paragraph::new(status_line.centered()), status);

        // A third of the way down, like a page.
        let box_height = NUM_LINES as u16 + 2;
        let top = body.y + 1 + body.height.saturating_sub(box_height + 1) / 3;
        let box_area = crate::ui::centered(
            Rect {
                y: top,
                height: box_height.min(body.height),
                ..body
            },
            width,
            box_height,
        );
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(s.border);
        let inner = block.inner(box_area);
        frame.render_widget(block, box_area);
        let inner = Rect {
            x: inner.x + 2,
            width: inner.width.saturating_sub(4),
            ..inner
        };
        for (i, line) in self.lines.iter().enumerate() {
            let chars: Vec<Char> = if i == self.active {
                self.input.output()
            } else if let Some(typed) = &self.typed[i] {
                typed.clone()
            } else {
                plain_chars(line)
            };
            let view = TextView {
                chars: &chars,
                styles: s,
                whitespace: settings.whitespace_style,
                caret: settings.caret_shape,
                rtl: false,
                typo: i == self.active && self.input.has_typo(),
            };
            let row = Rect {
                y: inner.y + i as u16,
                height: 1,
                ..inner
            };
            // Lines do not wrap; the active one scrolls to keep the cursor
            // in view when the terminal is narrow.
            if row.y < inner.y + inner.height
                && let Some(pos) = view.render_line(row, frame.buffer_mut(), i == self.active)
                && settings.caret_shape == crate::settings::CaretShape::Line
                && i == self.active
            {
                frame.set_cursor_position(pos);
            }
        }
    }

    fn render_report(
        &self,
        frame: &mut Frame,
        area: Rect,
        ctx: &RenderContext,
        report: &Report,
        history: &[TestResult],
    ) {
        let s = ctx.styles;
        let unit = ctx.settings.speed_unit;
        let r = &report.result;
        let big = s.value.add_modifier(Modifier::BOLD);
        let beats = Distribution::speed().cdf(r.speed) * 100.0;
        let previous: Vec<&TestResult> = history
            .iter()
            .filter(|t| t.time_stamp != r.time_stamp)
            .collect();
        let best = previous.iter().map(|t| t.speed).fold(0.0, f64::max);
        let recent: Vec<f64> = previous.iter().rev().take(10).map(|t| t.speed).collect();
        let average = if recent.is_empty() {
            0.0
        } else {
            recent.iter().sum::<f64>() / recent.len() as f64
        };
        let mut lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("Speed  ", s.name),
                Span::styled(unit.format(r.speed), big),
            ])
            .centered(),
            Line::from(vec![
                Span::styled("Accuracy  ", s.name),
                Span::styled(format!("{:.1}%", r.accuracy * 100.0), big),
                Span::styled("   Time  ", s.name),
                Span::styled(format_duration(r.time as u64), big),
                Span::styled("   Characters  ", s.name),
                Span::styled(thousands(r.length as f64), big),
                Span::styled("   Errors  ", s.name),
                Span::styled(r.errors.to_string(), big),
            ])
            .centered(),
            Line::from(""),
            Line::from(vec![
                Span::styled("This speed beats ", s.name),
                Span::styled(format!("{beats:.1}%"), s.value),
                Span::styled(" of all keybr.com users.", s.name),
            ])
            .centered(),
        ];
        if !previous.is_empty() {
            let delta = r.speed - average;
            lines.push(
                Line::from(vec![
                    Span::styled("Your best: ", s.name),
                    Span::styled(unit.format(best), s.value),
                    Span::styled(format!("   Average of the last {}: ", recent.len()), s.name),
                    Span::styled(unit.format(average), s.value),
                    Span::styled(
                        format!(" ({})", crate::ui::signed(delta, unit.format(delta))),
                        s.delta(delta),
                    ),
                ])
                .centered(),
            );
            if r.speed > best {
                lines.push(Line::from(Span::styled("A new personal best!", s.title)).centered());
            }
        }
        let [text, chart, _, hint] = Layout::vertical([
            Constraint::Length(lines.len() as u16 + 1),
            Constraint::Length(8),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area);
        frame.render_widget(Paragraph::new(lines), text);
        // Stretch the windows across the chart, like keybr's resampling.
        let columns = 58usize;
        let data: Vec<u64> = if report.speeds.is_empty() {
            Vec::new()
        } else {
            (0..columns)
                .map(|i| {
                    unit.measure(report.speeds[i * report.speeds.len() / columns])
                        .round() as u64
                })
                .collect()
        };
        let chart = crate::ui::centered(chart, 60, 8);
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(s.border)
            .title(Span::styled(
                format!(" Speed during the test ({}) ", unit.id()),
                s.name,
            ));
        frame.render_widget(
            Sparkline::default()
                .block(block)
                .data(&data)
                .style(Style::new().fg(s.color(s.palette.chart_speed))),
            chart,
        );
        frame.render_widget(
            Paragraph::new(
                Line::from(Span::styled(
                    "Press Enter or Esc to take a new test.",
                    s.accent,
                ))
                .centered(),
            ),
            hint,
        );
    }
}

/// The serialized name of a setting value, e.g. `seconds15`.
fn id_of<T: serde::Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

fn plain_chars(line: &str) -> Vec<Char> {
    line.chars()
        .map(|c| Char {
            cp: u32::from(c),
            attr: crate::textinput::Attr::Normal,
            cls: None,
        })
        .collect()
}

/// A line of words of at most `NUM_COLS` characters, ending with a space.
fn generate_line(source: &mut Source, rng: &mut Lcg) -> String {
    let mut line = String::new();
    let mut words = 0;
    loop {
        let state = (
            rng.clone(),
            match source {
                Source::Book { index, .. } => *index,
                _ => 0,
            },
        );
        let word = source.next_word(rng);
        // Measured in terminal columns: some scripts are two columns wide.
        if words > 0 && line.width() + word.width() + 1 > NUM_COLS {
            // Put the word back for the next line.
            *rng = state.0;
            if let Source::Book { index, .. } = source {
                *index = state.1;
            }
            break;
        }
        line.push_str(&word);
        line.push(' ');
        words += 1;
    }
    line
}

/// Typing speed in two second windows.
fn speed_series(steps: &[Step]) -> Vec<f64> {
    let Some(first) = steps.first() else {
        return Vec::new();
    };
    let window = 2000.0;
    let end = steps.last().map_or(0.0, |s| s.time_stamp);
    let buckets = (((end - first.time_stamp) / window).ceil() as usize).max(1);
    let mut counts = vec![0usize; buckets];
    for step in steps {
        let i = (((step.time_stamp - first.time_stamp) / window) as usize).min(buckets - 1);
        counts[i] += 1;
    }
    counts
        .into_iter()
        .map(|n| n as f64 / (window / 1000.0) * 60.0)
        .collect()
}
