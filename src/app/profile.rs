//! The profile screen: statistics and charts of your progress.

use std::collections::{BTreeSet, HashMap};

use chrono::{Datelike, Duration, Local, NaiveDate};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Axis, Bar, BarChart, BarGroup, Block, BorderType, Borders, Chart, Dataset, GraphType,
    Paragraph, Widget,
};

use super::RenderContext;
use crate::keyboard::{Keyboard, Layout as KeyboardLayout};
use crate::lesson::{Target, shared_model};
use crate::result::{KeyStatsMap, StreakList, SummaryStats, TypingResult, time_to_speed};
use crate::settings::{Settings, SpeedUnit};
use crate::stats::{
    CharClass, Distribution, key_usage, practice_days, progress_overview, smooth_series, week_start,
};
use crate::theme::Rgb;
use crate::ui::keyboard::KeyboardView;
use crate::ui::{Styles, format_duration, thousands};

/// What a key press on the profile screen asks the app to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileAction {
    None,
    Back,
    Export,
    Reset,
}

pub struct Profile {
    pub scroll: u16,
    /// From 0 to 1.
    pub smoothness: f64,
    pub char_class: CharClass,
    /// The layout whose key statistics are shown.
    pub layout: Option<String>,
    /// The key shown in the key speed chart.
    pub key_index: usize,
    /// Waiting for the reset to be confirmed.
    pub confirm_reset: bool,
    /// The height of the page when it was last drawn.
    page_height: std::cell::Cell<u16>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            scroll: 0,
            smoothness: 0.5,
            char_class: CharClass::Letters,
            layout: None,
            key_index: 0,
            confirm_reset: false,
            page_height: std::cell::Cell::new(0),
        }
    }
}

const WIDTH: u16 = 112;

/// A titled block of the page: its title, height and how to draw it.
type Section<'a> = (String, u16, Box<dyn Fn(Rect, &mut Buffer) + 'a>);

impl Profile {
    pub fn handle_key(
        &mut self,
        key: ratatui::crossterm::event::KeyEvent,
        results: &[TypingResult],
    ) -> ProfileAction {
        use ratatui::crossterm::event::KeyCode;
        if self.confirm_reset {
            self.confirm_reset = false;
            return if matches!(key.code, KeyCode::Char('y' | 'Y')) {
                ProfileAction::Reset
            } else {
                ProfileAction::None
            };
        }
        let max_scroll = self.page_height.get();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return ProfileAction::Back,
            KeyCode::Down | KeyCode::Char('j') => self.scroll = (self.scroll + 2).min(max_scroll),
            KeyCode::Up | KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(2),
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.scroll = (self.scroll + 20).min(max_scroll)
            }
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(20),
            KeyCode::Home | KeyCode::Char('g') => self.scroll = 0,
            KeyCode::End | KeyCode::Char('G') => self.scroll = max_scroll,
            KeyCode::Left => self.key_index = self.key_index.saturating_sub(1),
            KeyCode::Right => self.key_index += 1,
            KeyCode::Char('c') => {
                self.char_class = self.char_class.next();
                self.key_index = 0;
            }
            KeyCode::Char('l') => {
                let layouts = layouts_in(results);
                if !layouts.is_empty() {
                    let current = self
                        .layout
                        .as_ref()
                        .and_then(|l| layouts.iter().position(|x| x == l));
                    self.layout =
                        Some(layouts[current.map_or(0, |i| (i + 1) % layouts.len())].clone());
                    self.key_index = 0;
                }
            }
            KeyCode::Char('[') => self.smoothness = ((self.smoothness - 0.1) * 10.0).round() / 10.0,
            KeyCode::Char(']') => self.smoothness = ((self.smoothness + 0.1) * 10.0).round() / 10.0,
            KeyCode::Char('e') => return ProfileAction::Export,
            KeyCode::Char('R') => self.confirm_reset = true,
            _ => {}
        }
        self.smoothness = self.smoothness.clamp(0.0, 1.0);
        ProfileAction::None
    }

    /// The layout whose keys are shown: the chosen one, the configured one
    /// if it has results, or the first one with results.
    fn selected_layout(&self, results: &[TypingResult], settings: &Settings) -> String {
        let layouts = layouts_in(results);
        if let Some(l) = &self.layout
            && layouts.contains(l)
        {
            return l.clone();
        }
        if layouts.contains(&settings.layout) || layouts.is_empty() {
            settings.layout.clone()
        } else {
            layouts[0].clone()
        }
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        ctx: &RenderContext,
        results: &[TypingResult],
    ) {
        let width = area.width.min(WIDTH);
        let page = self.page(width, ctx, results);
        let page_height = page.area.height;
        let visible = area.height;
        self.page_height.set(page_height.saturating_sub(visible));
        let scroll = self.scroll.min(page_height.saturating_sub(visible));
        let x0 = area.x + (area.width - width) / 2;
        let buf = frame.buffer_mut();
        for y in 0..visible.min(page_height.saturating_sub(scroll)) {
            for x in 0..width {
                if let Some(cell) = buf.cell_mut((x0 + x, area.y + y)) {
                    *cell = page[(x, y + scroll)].clone();
                }
            }
        }
        if self.confirm_reset {
            let dialog = crate::ui::centered(area, 60, 7);
            frame.render_widget(ratatui::widgets::Clear, dialog);
            let text = vec![
                Line::from(""),
                Line::from(Span::styled("Reset all statistics?", ctx.styles.value)).centered(),
                Line::from(Span::styled(
                    "Your results are moved to a backup file in the data folder.",
                    ctx.styles.name,
                ))
                .centered(),
                Line::from(Span::styled(
                    "Press y to reset, any other key to cancel.",
                    ctx.styles.accent,
                ))
                .centered(),
            ];
            let block = Block::new()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(ctx.styles.error)
                .style(ctx.styles.base);
            frame.render_widget(Paragraph::new(text).block(block), dialog);
        }
    }

    /// Renders the whole page into a buffer which is then scrolled.
    fn page(&self, width: u16, ctx: &RenderContext, results: &[TypingResult]) -> Buffer {
        let s = ctx.styles;
        let settings = ctx.settings;
        let unit = settings.speed_unit;
        let inner = width.saturating_sub(4);

        // Key statistics for the chosen layout and class of characters.
        let layout_id = self.selected_layout(results, settings);
        let layout = KeyboardLayout::get(&layout_id);
        let keyboard = Keyboard::new(layout, &layout.geometries[0], "standard");
        let code_points = keyboard.code_points();
        let model = shared_model(layout.language());
        let language_letters: Vec<_> = model
            .letters
            .iter()
            .filter(|l| code_points.has(l.cp))
            .cloned()
            .collect();
        let letters =
            crate::phonetic::Letter::frequency_order(&self.char_class.letters(&language_letters));
        let layout_results: Vec<&TypingResult> =
            results.iter().filter(|r| r.layout == layout_id).collect();
        let key_stats = KeyStatsMap::from_results(&letters, layout_results.iter().copied());
        let target = Target::new(settings);

        let mut sections: Vec<Section> = Vec::new();

        let all = SummaryStats::from_results(results);
        let today = Local::now().date_naive();
        let today_stats =
            SummaryStats::from_results(results.iter().filter(|r| r.local_date() == today));
        sections.push((
            "Statistics".into(),
            8,
            Box::new(move |area, buf| {
                let [left, right] =
                    Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                        .areas(area);
                summary(s, unit, "All Time Statistics", &all, None).render(left, buf);
                let goal = (settings.daily_goal > 0).then_some(ctx.daily_goal);
                summary(s, unit, "Statistics for Today", &today_stats, goal).render(right, buf);
            }),
        ));

        let streaks = StreakList::find_longest(results);
        let streak_height = if streaks.is_empty() {
            2
        } else {
            streaks.len() as u16 + 1
        };
        sections.push((
            "Accuracy Streaks".into(),
            streak_height,
            Box::new(move |area, buf| streak_table(s, unit, &streaks).render(area, buf)),
        ));

        sections.push((
            "Relative Typing Speed".into(),
            12,
            Box::new(move |area, buf| {
                let dist = Distribution::speed();
                let max = dist.len() as f64 - 1.0;
                let markers = [
                    (all.speed.avg, s.palette.chart_speed, "average"),
                    (all.speed.max, s.palette.chart_threshold, "top"),
                ];
                histogram(s, area, buf, dist, 0.0, max, &markers, &|i| unit.format(i));
                let line = |label: &str, value: f64| {
                    let beats = if value > 0.0 {
                        format!("{:.1}%", dist.cdf(value) * 100.0)
                    } else {
                        "N/A".into()
                    };
                    Line::from(vec![
                        Span::styled(
                            format!(
                                "Your all time {label} speed ({}) beats ",
                                unit.format(value)
                            ),
                            s.name,
                        ),
                        Span::styled(beats, s.value),
                        Span::styled(" of all other people.", s.name),
                    ])
                };
                let y = area.y + area.height - 2;
                Paragraph::new(vec![
                    line("top", all.speed.max),
                    line("average", all.speed.avg),
                ])
                .render(
                    Rect {
                        y,
                        height: 2,
                        ..area
                    },
                    buf,
                );
            }),
        ));

        sections.push((
            "Relative Accuracy".into(),
            12,
            Box::new(move |area, buf| {
                let dist = Distribution::accuracy();
                let scale = dist.len() as f64 - 1.0;
                let markers = [
                    (all.accuracy.avg * scale, s.palette.chart_speed, "average"),
                    (all.accuracy.max * scale, s.palette.chart_threshold, "top"),
                ];
                histogram(s, area, buf, dist, scale * 0.75, scale, &markers, &|i| {
                    format!("{:.0}%", i / scale * 100.0)
                });
                let line = |label: &str, value: f64| {
                    let beats = if value > 0.0 {
                        format!("{:.1}%", dist.cdf(dist.scale(value)) * 100.0)
                    } else {
                        "N/A".into()
                    };
                    Line::from(vec![
                        Span::styled(
                            format!(
                                "Your all time {label} accuracy ({:.1}%) beats ",
                                value * 100.0
                            ),
                            s.name,
                        ),
                        Span::styled(beats, s.value),
                        Span::styled(" of all other people.", s.name),
                    ])
                };
                let y = area.y + area.height - 2;
                Paragraph::new(vec![
                    line("top", all.accuracy.max),
                    line("average", all.accuracy.avg),
                ])
                .render(
                    Rect {
                        y,
                        height: 2,
                        ..area
                    },
                    buf,
                );
            }),
        ));

        let smoothness = self.smoothness;
        sections.push((
            format!("Typing Speed  [ ] smoothness {smoothness:.1}"),
            30,
            Box::new(move |area, buf| {
                speed_charts(s, unit, settings, results, smoothness, area, buf)
            }),
        ));

        let selector = format!(
            "Show statistics for: {} (l), {} (c)",
            layout.full_name(),
            self.char_class.name()
        );
        sections.push((
            "Learning Progress Overview".into(),
            key_stats.iter().count() as u16 + 3,
            Box::new({
                let key_stats = &key_stats;
                let selector = selector.clone();
                move |area, buf| {
                    Paragraph::new(Line::from(Span::styled(selector.clone(), s.accent)))
                        .render(area, buf);
                    let chart = Rect {
                        y: area.y + 2,
                        height: area.height - 2,
                        ..area
                    };
                    overview(s, key_stats, &target, chart, buf);
                }
            }),
        ));

        let keys: Vec<_> = key_stats.iter().collect();
        let key_index = self.key_index.min(keys.len().saturating_sub(1));
        let selected = keys.get(key_index).map(|k| (*k).clone());
        sections.push((
            format!(
                "Key Typing Speed: {}  ← →",
                selected
                    .as_ref()
                    .map_or("-".to_string(), |k| k.letter.label.clone())
            ),
            14,
            Box::new(move |area, buf| {
                key_speed_chart(s, unit, &target, selected.as_ref(), area, buf)
            }),
        ));

        sections.push((
            "Key Typing Speed Histogram".into(),
            14,
            Box::new({
                let key_stats = &key_stats;
                move |area, buf| key_speed_histogram(s, unit, &target, key_stats, area, buf)
            }),
        ));

        sections.push((
            "Key Frequency Histogram".into(),
            20,
            Box::new({
                let key_stats = &key_stats;
                move |area, buf| key_frequency(s, key_stats, area, buf)
            }),
        ));

        let (kb_w, kb_h) = KeyboardView::size(&keyboard, true);
        let side_by_side = inner >= kb_w * 2 + 4;
        let heatmap_height = if side_by_side { kb_h + 2 } else { kb_h * 2 + 4 };
        sections.push((
            "Key Frequency Heatmap".into(),
            heatmap_height,
            Box::new({
                let key_stats = &key_stats;
                let keyboard = &keyboard;
                move |area, buf| heatmap(s, keyboard, key_stats, side_by_side, area, buf)
            }),
        ));

        let days = practice_days(results);
        sections.push((
            "Practice Calendar".into(),
            11,
            Box::new(move |area, buf| calendar(s, &days, settings.daily_goal, today, area, buf)),
        ));

        let footer = format!(
            "Your data is in {}.  Press e to export it as JSON, R to reset statistics.",
            crate::app::short_path(&ctx.data_dir)
        );
        sections.push((
            "Data".into(),
            2,
            Box::new(move |area, buf| {
                Paragraph::new(Span::styled(footer.clone(), s.name))
                    .wrap(ratatui::widgets::Wrap { trim: true })
                    .render(area, buf)
            }),
        ));

        let total: u16 = sections.iter().map(|(_, h, _)| h + 3).sum();
        let mut buf = Buffer::empty(Rect::new(0, 0, width, total));
        buf.set_style(buf.area, s.base);
        let mut y = 0;
        for (title, height, draw) in &sections {
            let area = Rect::new(0, y, width, height + 2);
            let block = Block::new()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(s.border)
                .title(Span::styled(format!(" {title} "), s.title));
            let inner_area = block.inner(area);
            block.render(area, &mut buf);
            let inner_area = Rect {
                x: inner_area.x + 1,
                width: inner_area.width.saturating_sub(2),
                ..inner_area
            };
            draw(inner_area, &mut buf);
            y += height + 3;
        }
        buf
    }
}

/// Layouts that have results, most used first.
fn layouts_in(results: &[TypingResult]) -> Vec<String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for r in results {
        *counts.entry(r.layout.as_str()).or_default() += 1;
    }
    let mut layouts: Vec<(&str, usize)> = counts.into_iter().collect();
    layouts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    layouts.into_iter().map(|(l, _)| l.to_string()).collect()
}

fn summary<'a>(
    s: &Styles,
    unit: SpeedUnit,
    title: &'a str,
    stats: &SummaryStats,
    goal: Option<crate::result::DailyGoal>,
) -> Paragraph<'a> {
    let row = |name: &str, value: String| {
        Line::from(vec![
            Span::styled(format!("{name:<18}"), s.name),
            Span::styled(value, s.value),
        ])
    };
    let none = || "-".to_string();
    let has = stats.count > 0;
    let mut lines = vec![
        Line::from(Span::styled(title, s.text.add_modifier(Modifier::BOLD))),
        row("Lessons", thousands(stats.count as f64)),
        row("Time", format_duration(stats.time)),
        row(
            "Top speed",
            if has {
                unit.format(stats.speed.max)
            } else {
                none()
            },
        ),
        row(
            "Average speed",
            if has {
                unit.format(stats.speed.avg)
            } else {
                none()
            },
        ),
        row(
            "Top accuracy",
            if has {
                format!("{:.1}%", stats.accuracy.max * 100.0)
            } else {
                none()
            },
        ),
        row(
            "Average accuracy",
            if has {
                format!("{:.1}%", stats.accuracy.avg * 100.0)
            } else {
                none()
            },
        ),
    ];
    if let Some(goal) = goal {
        lines.push(row(
            "Daily goal",
            format!("{:.0}% of {} minutes", goal.value * 100.0, goal.goal),
        ));
    }
    Paragraph::new(lines)
}

fn streak_table<'a>(
    s: &Styles,
    unit: SpeedUnit,
    streaks: &[crate::result::Streak],
) -> Paragraph<'a> {
    if streaks.is_empty() {
        return Paragraph::new(Span::styled(
            "You don't have any accuracy streaks. Consider completing a lesson with the highest accuracy \
             possible, regardless of typing speed.",
            s.name,
        ))
        .wrap(ratatui::widgets::Wrap { trim: true });
    }
    let header = format!(
        "{:<12}{:>9}  {:<14}{:>16}{:>14}{:>14}",
        "Accuracy", "Lessons", "Start date", "Average speed", "Top speed", "Characters"
    );
    let mut lines = vec![Line::from(Span::styled(header, s.name))];
    for streak in streaks {
        let n = streak.results.len();
        let avg = streak.results.iter().map(|r| r.speed).sum::<f64>() / n as f64;
        let top = streak.results.iter().map(|r| r.speed).fold(0.0, f64::max);
        let chars: u64 = streak.results.iter().map(|r| r.length as u64).sum();
        let start = crate::result::local_date(streak.results[0].time_stamp);
        lines.push(Line::from(Span::styled(
            format!(
                "{:<12}{:>9}  {:<14}{:>16}{:>14}{:>14}",
                format!("≥ {:.0}%", streak.level * 100.0),
                n,
                start.format("%Y-%m-%d").to_string(),
                unit.format(avg),
                unit.format(top),
                thousands(chars as f64),
            ),
            s.value,
        )));
    }
    Paragraph::new(lines)
}

/// Draws the histogram of a distribution from index `lo` to `hi`, with
/// vertical markers for the user's values.
#[allow(clippy::too_many_arguments)]
fn histogram(
    s: &Styles,
    area: Rect,
    buf: &mut Buffer,
    dist: &Distribution,
    lo: f64,
    hi: f64,
    markers: &[(f64, Rgb, &str)],
    label: &dyn Fn(f64) -> String,
) {
    let chart_height = area.height.saturating_sub(4) as usize;
    let cols = area.width as usize;
    if cols == 0 || chart_height == 0 {
        return;
    }
    let col_value = |c: usize| lo + (hi - lo) * c as f64 / (cols - 1).max(1) as f64;
    let values: Vec<f64> = (0..cols)
        .map(|c| {
            let a = col_value(c);
            let b = col_value(c + 1);
            let n = ((b - a).round() as usize).max(1);
            (0..n).map(|i| dist.pmf(a + i as f64)).sum::<f64>() / n as f64
        })
        .collect();
    let max = values.iter().cloned().fold(0.0, f64::max).max(1e-12);
    let blocks = [" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
    let bar_style = Style::new().fg(s.color(s.palette.chart_hits));
    for (c, v) in values.iter().enumerate() {
        let eighths = (v / max * (chart_height * 8) as f64).round() as usize;
        for row in 0..chart_height {
            let filled = eighths.saturating_sub(row * 8).min(8);
            let y = area.y + (chart_height - 1 - row) as u16;
            buf.set_string(area.x + c as u16, y, blocks[filled], bar_style);
        }
    }
    for (value, color, _) in markers {
        if *value <= 0.0 {
            continue;
        }
        let c = (((value - lo) / (hi - lo)) * (cols - 1) as f64)
            .round()
            .clamp(0.0, (cols - 1) as f64) as u16;
        for row in 0..chart_height as u16 {
            buf.set_string(
                area.x + c,
                area.y + row,
                "│",
                Style::new()
                    .fg(s.color(*color))
                    .add_modifier(Modifier::BOLD),
            );
        }
    }
    // Axis labels.
    let y = area.y + chart_height as u16;
    for i in 0..5 {
        let c = i * (cols - 1) / 4;
        let text = label(col_value(c));
        let x = (area.x + c as u16).saturating_sub(if i == 4 {
            text.len() as u16 - 1
        } else if i > 0 {
            text.len() as u16 / 2
        } else {
            0
        });
        buf.set_string(x, y, &text, s.name);
    }
    let mut legend = vec![];
    for (_, color, name) in markers {
        legend.push(Span::styled("│ ", Style::new().fg(s.color(*color))));
        legend.push(Span::styled(format!("your {name}   "), s.name));
    }
    Paragraph::new(Line::from(legend)).render(
        Rect {
            y: y + 1,
            height: 1,
            ..area
        },
        buf,
    );
}

fn axis_labels(lo: f64, hi: f64, format: &dyn Fn(f64) -> String) -> Vec<Line<'static>> {
    vec![
        Line::from(format(lo)),
        Line::from(format((lo + hi) / 2.0)),
        Line::from(format(hi)),
    ]
}

fn speed_charts(
    s: &Styles,
    unit: SpeedUnit,
    settings: &Settings,
    results: &[TypingResult],
    smoothness: f64,
    area: Rect,
    buf: &mut Buffer,
) {
    if results.len() < 2 {
        Paragraph::new(Span::styled(
            "Complete a few lessons to see your progress over time.",
            s.name,
        ))
        .render(area, buf);
        return;
    }
    let n = results.len() as f64;
    let xs: Vec<f64> = (1..=results.len()).map(|i| i as f64).collect();
    let speed = smooth_series(results.iter().map(|r| unit.measure(r.speed)), smoothness);
    let accuracy = smooth_series(results.iter().map(|r| r.accuracy * 100.0), smoothness);
    let complexity = smooth_series(results.iter().map(|r| r.complexity as f64), smoothness);
    let [a, b, c] = Layout::vertical([
        Constraint::Fill(2),
        Constraint::Fill(1),
        Constraint::Fill(1),
    ])
    .areas(area);
    let target = unit.measure(settings.target_speed as f64);
    let x_axis = Axis::default()
        .bounds([1.0, n])
        .labels(vec![
            Line::from("1"),
            Line::from(format!("{}", (n / 2.0).round())),
            Line::from(format!("{n}")),
        ])
        .style(s.name);

    let points =
        |ys: &[f64]| -> Vec<(f64, f64)> { xs.iter().copied().zip(ys.iter().copied()).collect() };
    let speed_points = points(&speed);
    let target_points = vec![(1.0, target), (n, target)];
    let max_speed = speed.iter().cloned().fold(target, f64::max) * 1.1;
    let min_speed = speed.iter().cloned().fold(target, f64::min) * 0.9;
    let fmt = |v: f64| format!("{v:.0}{}", unit.id());
    Chart::new(vec![
        Dataset::default()
            .name(format!("speed ({})", unit.id()))
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::new().fg(s.color(s.palette.chart_speed)))
            .data(&speed_points),
        Dataset::default()
            .name("target")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::new().fg(s.color(s.palette.chart_threshold)))
            .data(&target_points),
    ])
    .x_axis(x_axis.clone())
    .y_axis(
        Axis::default()
            .bounds([min_speed, max_speed])
            .labels(axis_labels(min_speed, max_speed, &fmt))
            .style(s.name),
    )
    .legend_position(Some(ratatui::widgets::LegendPosition::TopLeft))
    .hidden_legend_constraints((Constraint::Ratio(1, 2), Constraint::Ratio(1, 1)))
    .style(s.base)
    .render(a, buf);

    let acc_points = points(&accuracy);
    let min_acc = accuracy
        .iter()
        .cloned()
        .fold(100.0, f64::min)
        .floor()
        .min(95.0);
    Chart::new(vec![
        Dataset::default()
            .name("accuracy (%)")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::new().fg(s.color(s.palette.chart_accuracy)))
            .data(&acc_points),
    ])
    .x_axis(x_axis.clone())
    .y_axis(
        Axis::default()
            .bounds([min_acc, 100.0])
            .labels(axis_labels(min_acc, 100.0, &|v| format!("{v:.0}%")))
            .style(s.name),
    )
    .legend_position(Some(ratatui::widgets::LegendPosition::TopLeft))
    .hidden_legend_constraints((Constraint::Ratio(1, 2), Constraint::Ratio(1, 1)))
    .style(s.base)
    .render(b, buf);

    let keys_points = points(&complexity);
    let max_keys = complexity.iter().cloned().fold(1.0, f64::max).ceil() + 1.0;
    Chart::new(vec![
        Dataset::default()
            .name("keys in the lesson")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::new().fg(s.color(s.palette.chart_complexity)))
            .data(&keys_points),
    ])
    .x_axis(x_axis)
    .y_axis(
        Axis::default()
            .bounds([0.0, max_keys])
            .labels(axis_labels(0.0, max_keys, &|v| format!("{v:.0}")))
            .style(s.name),
    )
    .legend_position(Some(ratatui::widgets::LegendPosition::TopLeft))
    .hidden_legend_constraints((Constraint::Ratio(1, 2), Constraint::Ratio(1, 1)))
    .style(s.base)
    .render(c, buf);
}

fn overview(s: &Styles, stats: &KeyStatsMap, target: &Target, area: Rect, buf: &mut Buffer) {
    if stats.result_count == 0 {
        Paragraph::new(Span::styled("No lessons typed on this layout yet.", s.name))
            .render(area, buf);
        return;
    }
    let label_width = 4u16;
    let width = area.width.saturating_sub(label_width) as usize;
    for (row, (letter, columns)) in progress_overview(stats, target, width)
        .into_iter()
        .enumerate()
    {
        let y = area.y + row as u16;
        if y >= area.y + area.height.saturating_sub(1) {
            break;
        }
        buf.set_string(area.x, y, format!("{:>2}", letter.label), s.value);
        for (c, confidence) in columns.into_iter().enumerate() {
            if let Some(conf) = confidence {
                let style = Style::new().fg(s.color(s.confidence_rgb(conf)));
                buf.set_string(area.x + label_width + c as u16, y, "█", style);
            } else {
                buf.set_string(area.x + label_width + c as u16, y, "·", s.dim);
            }
        }
    }
    let y = area.y + area.height - 1;
    let legend = Line::from(vec![
        Span::styled("lessons →   ", s.name),
        Span::styled("█", Style::new().fg(s.color(s.palette.slow_key))),
        Span::styled(" slow   ", s.name),
        Span::styled("█", Style::new().fg(s.color(s.palette.fast_key))),
        Span::styled(" at the target speed", s.name),
    ]);
    Paragraph::new(legend).render(
        Rect {
            y,
            height: 1,
            ..area
        },
        buf,
    );
}

fn key_speed_chart(
    s: &Styles,
    unit: SpeedUnit,
    target: &Target,
    key: Option<&crate::result::KeyStats>,
    area: Rect,
    buf: &mut Buffer,
) {
    let Some(key) = key.filter(|k| k.samples.len() >= 2) else {
        Paragraph::new(Span::styled("Not enough data for this key yet.", s.name)).render(area, buf);
        return;
    };
    let points: Vec<(f64, f64)> = key
        .samples
        .iter()
        .map(|k| {
            (
                k.index as f64 + 1.0,
                unit.measure(time_to_speed(k.filtered_time_to_type)),
            )
        })
        .collect();
    let first = points[0].0;
    let last = points[points.len() - 1].0;
    let goal = unit.measure(target.target_speed);
    let target_points = vec![(first, goal), (last, goal)];
    let max = points.iter().map(|p| p.1).fold(goal, f64::max) * 1.1;
    let min = points.iter().map(|p| p.1).fold(goal, f64::min) * 0.9;
    let fmt = |v: f64| format!("{v:.0}{}", unit.id());
    Chart::new(vec![
        Dataset::default()
            .name(format!("{} speed", key.letter.label))
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::new().fg(s.color(s.palette.chart_speed)))
            .data(&points),
        Dataset::default()
            .name("target")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::new().fg(s.color(s.palette.chart_threshold)))
            .data(&target_points),
    ])
    .x_axis(
        Axis::default()
            .bounds([first, last])
            .labels(vec![
                Line::from(format!("{first}")),
                Line::from(format!("{last}")),
            ])
            .style(s.name),
    )
    .y_axis(
        Axis::default()
            .bounds([min, max])
            .labels(axis_labels(min, max, &fmt))
            .style(s.name),
    )
    .legend_position(Some(ratatui::widgets::LegendPosition::TopLeft))
    .hidden_legend_constraints((Constraint::Ratio(1, 2), Constraint::Ratio(1, 1)))
    .style(s.base)
    .render(area, buf);
}

/// The bar width and gap which fit `count` bars in the width.
fn bar_layout(count: usize, width: u16) -> (u16, u16) {
    let per = (width as usize / count.max(1)).clamp(1, 6) as u16;
    if per >= 2 { (per - 1, 1) } else { (1, 0) }
}

fn key_speed_histogram(
    s: &Styles,
    unit: SpeedUnit,
    target: &Target,
    stats: &KeyStatsMap,
    area: Rect,
    buf: &mut Buffer,
) {
    let keys: Vec<_> = stats.iter().collect();
    if keys.iter().all(|k| k.time_to_type.is_none()) {
        Paragraph::new(Span::styled("No key speeds measured yet.", s.name)).render(area, buf);
        return;
    }
    let (bar_width, gap) = bar_layout(keys.len(), area.width);
    let bars: Vec<Bar> = keys
        .iter()
        .map(|k| {
            let speed = k
                .time_to_type
                .map_or(0.0, |t| unit.measure(time_to_speed(t)));
            let confidence = target.confidence(k.time_to_type).unwrap_or(0.0);
            let color = s.color(s.confidence_rgb(confidence));
            Bar::default()
                .value((speed * 10.0).round() as u64)
                .text_value(if bar_width >= 3 {
                    format!("{speed:.0}")
                } else {
                    String::new()
                })
                .label(Line::from(k.letter.label.clone()))
                .style(Style::new().fg(color))
                .value_style(s.on(s.confidence_rgb(confidence)))
        })
        .collect();
    BarChart::default()
        .data(BarGroup::default().bars(&bars))
        .bar_width(bar_width)
        .bar_gap(gap)
        .label_style(s.value)
        .style(s.base)
        .render(
            Rect {
                height: area.height - 1,
                ..area
            },
            buf,
        );
    let line = Line::from(Span::styled(
        format!(
            "Bars: the current speed of every key in {}, colored from slow to the target speed.",
            unit.id()
        ),
        s.name,
    ));
    Paragraph::new(line).render(
        Rect {
            y: area.y + area.height - 1,
            height: 1,
            ..area
        },
        buf,
    );
}

fn key_frequency(s: &Styles, stats: &KeyStatsMap, area: Rect, buf: &mut Buffer) {
    let usage = key_usage(stats);
    if usage.iter().all(|u| u.hits == 0) {
        Paragraph::new(Span::styled("No keys typed yet.", s.name)).render(area, buf);
        return;
    }
    let (bar_width, gap) = bar_layout(usage.len(), area.width);
    let [top, bottom, legend] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);
    let hits: Vec<Bar> = usage
        .iter()
        .map(|u| {
            Bar::default()
                .value(u.hits as u64)
                .text_value(String::new())
                .label(Line::from(u.letter.label.clone()))
                .style(Style::new().fg(s.color(s.palette.chart_hits)))
        })
        .collect();
    BarChart::default()
        .data(BarGroup::default().bars(&hits))
        .bar_width(bar_width)
        .bar_gap(gap)
        .label_style(s.value)
        .style(s.base)
        .render(top, buf);
    let ratios: Vec<Bar> = usage
        .iter()
        .map(|u| {
            let ratio = u.ratio.unwrap_or(0.0) * 100.0;
            Bar::default()
                .value((ratio * 10.0).round() as u64)
                .text_value(String::new())
                .label(Line::from(u.letter.label.clone()))
                .style(Style::new().fg(s.color(s.palette.chart_ratio)))
        })
        .collect();
    BarChart::default()
        .data(BarGroup::default().bars(&ratios))
        .bar_width(bar_width)
        .bar_gap(gap)
        .label_style(s.value)
        .style(s.base)
        .render(bottom, buf);
    let worst = usage
        .iter()
        .filter_map(|u| u.ratio.map(|r| (r, u)))
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(r, u)| format!("  Most missed: {} ({:.1}%).", u.letter.label, r * 100.0))
        .unwrap_or_default();
    Paragraph::new(Line::from(vec![
        Span::styled("█", Style::new().fg(s.color(s.palette.chart_hits))),
        Span::styled(" hit count   ", s.name),
        Span::styled("█", Style::new().fg(s.color(s.palette.chart_ratio))),
        Span::styled(" miss/hit ratio (relative miss frequency)", s.name),
        Span::styled(worst, s.value),
    ]))
    .render(legend, buf);
}

fn heatmap(
    s: &Styles,
    keyboard: &Keyboard,
    stats: &KeyStatsMap,
    side_by_side: bool,
    area: Rect,
    buf: &mut Buffer,
) {
    let usage = key_usage(stats);
    let max_hits = usage.iter().map(|u| u.hits).max().unwrap_or(0).max(1) as f64;
    let max_misses = usage.iter().map(|u| u.misses).max().unwrap_or(0).max(1) as f64;
    let base = if s.themed {
        s.palette.background
    } else {
        Rgb(40, 40, 40)
    };
    let mut hits = HashMap::new();
    let mut misses = HashMap::new();
    for u in &usage {
        if let Some(combo) = keyboard.combo(u.letter.cp) {
            if u.hits > 0 {
                let t = (u.hits as f64 / max_hits).sqrt();
                hits.insert(
                    combo.key.clone(),
                    base.lerp(s.palette.chart_hits, 0.2 + 0.8 * t),
                );
            }
            if u.misses > 0 {
                let t = (u.misses as f64 / max_misses).sqrt();
                misses.insert(
                    combo.key.clone(),
                    base.lerp(s.palette.chart_misses, 0.2 + 0.8 * t),
                );
            }
        }
    }
    let (kb_w, kb_h) = KeyboardView::size(keyboard, true);
    let (a, b) = if side_by_side {
        let [a, b] =
            Layout::horizontal([Constraint::Length(kb_w + 2), Constraint::Length(kb_w + 2)])
                .flex(ratatui::layout::Flex::SpaceAround)
                .areas(area);
        (a, b)
    } else {
        let [a, b] = Layout::vertical([Constraint::Length(kb_h + 2), Constraint::Length(kb_h + 2)])
            .areas(area);
        (a, b)
    };
    for (rect, title, colors) in [(a, "Hit count", hits), (b, "Miss count", misses)] {
        Paragraph::new(Line::from(Span::styled(title, s.value)).centered())
            .render(Rect { height: 1, ..rect }, buf);
        let mut view = KeyboardView::new(keyboard, s);
        view.heat = Some(colors);
        view.zone_colors = false;
        view.render(
            Rect {
                y: rect.y + 1,
                height: rect.height.saturating_sub(1),
                ..rect
            },
            buf,
        );
    }
}

fn calendar(
    s: &Styles,
    days: &std::collections::BTreeMap<NaiveDate, u64>,
    goal: u32,
    today: NaiveDate,
    area: Rect,
    buf: &mut Buffer,
) {
    let weeks = ((area.width.saturating_sub(4)) / 2).min(53) as i64;
    let last_week = week_start(today);
    let first_week = last_week - Duration::weeks(weeks - 1);
    let goal_ms = (goal.max(1) as f64) * 60_000.0;
    let base = if s.themed {
        s.palette.panel
    } else {
        Rgb(60, 60, 60)
    };
    let full = s.palette.fast_key;
    let x0 = area.x + 4;
    // Month labels.
    let mut last_month = 0;
    for w in 0..weeks {
        let date = first_week + Duration::weeks(w);
        if date.month() != last_month {
            last_month = date.month();
            buf.set_string(
                x0 + (w * 2) as u16,
                area.y,
                date.format("%b").to_string(),
                s.name,
            );
        }
    }
    for (i, name) in ["Mon", "", "Wed", "", "Fri", "", "Sun"].iter().enumerate() {
        buf.set_string(area.x, area.y + 1 + i as u16, name, s.name);
    }
    let mut active_days = 0;
    let mut total = 0;
    for w in 0..weeks {
        for d in 0..7 {
            let date = first_week + Duration::weeks(w) + Duration::days(d);
            if date > today {
                continue;
            }
            let time = days.get(&date).copied().unwrap_or(0);
            let style = if time > 0 {
                active_days += 1;
                total += time;
                let t = (time as f64 / goal_ms).clamp(0.0, 1.0);
                Style::new().fg(s.color(base.lerp(full, 0.3 + 0.7 * t)))
            } else {
                Style::new().fg(s.color(base))
            };
            let symbol = if date == today { "◆" } else { "■" };
            buf.set_string(x0 + (w * 2) as u16, area.y + 1 + d as u16, symbol, style);
        }
    }
    let streak = current_streak(days, today);
    let text = format!(
        "{} active days, {} in the last {} weeks.  Current streak: {} day{}.",
        active_days,
        format_duration(total),
        weeks,
        streak,
        if streak == 1 { "" } else { "s" }
    );
    buf.set_string(area.x, area.y + 9, text, s.name);
    let mut x = area.x + area.width.saturating_sub(24);
    buf.set_string(x, area.y + 10, "Less ", s.name);
    x += 5;
    for t in [0.0, 0.3, 0.6, 1.0] {
        let color = if t == 0.0 {
            base
        } else {
            base.lerp(full, 0.3 + 0.7 * t)
        };
        buf.set_string(x, area.y + 10, "■ ", Style::new().fg(s.color(color)));
        x += 2;
    }
    buf.set_string(x, area.y + 10, "More", s.name);
}

/// Consecutive days with practice, up to today or yesterday.
pub fn current_streak(days: &std::collections::BTreeMap<NaiveDate, u64>, today: NaiveDate) -> u32 {
    let active: BTreeSet<NaiveDate> = days
        .iter()
        .filter(|(_, t)| **t > 0)
        .map(|(d, _)| *d)
        .collect();
    let mut day = if active.contains(&today) {
        today
    } else {
        today - Duration::days(1)
    };
    let mut n = 0;
    while active.contains(&day) {
        n += 1;
        day -= Duration::days(1);
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::ColorMode;
    use crate::theme::Theme;
    use std::collections::BTreeMap;

    fn page_text(profile: &Profile, results: &[TypingResult], width: u16) -> String {
        let styles = Styles::new(Theme::get("dark"), ColorMode::Truecolor);
        let settings = Settings::default();
        let ctx = RenderContext {
            styles: &styles,
            settings: &settings,
            daily_goal: crate::result::DailyGoal {
                goal: 30,
                value: 0.5,
            },
            toast: None,
            now: 0.0,
            focused: true,
            data_dir: std::path::PathBuf::from("/data"),
        };
        let buf = profile.page(width, &ctx, results);
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Forty lessons over the last ten days, getting faster.
    fn history() -> Vec<TypingResult> {
        let day = 86_400_000;
        let now = chrono::Utc::now().timestamp_millis();
        (0..40i64)
            .map(|i| {
                let samples: Vec<(char, u32)> =
                    "eniarlt".chars().map(|c| (c, 300 - i as u32 * 3)).collect();
                let accuracy = if i % 7 == 0 { 0.9 } else { 1.0 };
                let ts = now - (40 - i) * day / 4;
                crate::result::tests::result(ts, 150.0 + i as f64 * 5.0, accuracy, &samples)
            })
            .collect()
    }

    #[test]
    fn renders_every_section() {
        let text = page_text(&Profile::default(), &history(), 110);
        for title in [
            "All Time Statistics",
            "Statistics for Today",
            "Accuracy Streaks",
            "Relative Typing Speed",
            "Relative Accuracy",
            "Typing Speed",
            "Learning Progress Overview",
            "Key Typing Speed: E",
            "Key Typing Speed Histogram",
            "Key Frequency Histogram",
            "Hit count",
            "Miss count",
            "Practice Calendar",
            "beats",
            "/data",
        ] {
            assert!(text.contains(title), "{title} missing from\n{text}");
        }
        assert!(text.contains("Lessons           40"), "{text}");
    }

    #[test]
    fn renders_without_results_and_in_narrow_terminals() {
        let text = page_text(&Profile::default(), &[], 60);
        assert!(
            text.contains("No lessons typed on this layout yet."),
            "{text}"
        );
        assert!(text.contains("You don't have any accuracy"), "{text}");
        assert!(text.contains("Complete a few lessons"), "{text}");
        let text = page_text(&Profile::default(), &history(), 50);
        assert!(text.contains("Practice Calendar"));
    }

    #[test]
    fn keys_change_the_view() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let results = history();
        let mut profile = Profile::default();
        let key = |c| KeyEvent::new(c, KeyModifiers::NONE);
        profile.handle_key(key(KeyCode::Right), &results);
        assert!(page_text(&profile, &results, 110).contains("Key Typing Speed: N"));
        profile.handle_key(key(KeyCode::Char('c')), &results);
        assert_eq!(profile.char_class, CharClass::Digits);
        profile.handle_key(key(KeyCode::Char(']')), &results);
        assert!((profile.smoothness - 0.6).abs() < 1e-9);
        assert_eq!(
            profile.handle_key(key(KeyCode::Char('e')), &results),
            ProfileAction::Export
        );
        assert_eq!(
            profile.handle_key(key(KeyCode::Char('R')), &results),
            ProfileAction::None
        );
        assert!(profile.confirm_reset);
        assert_eq!(
            profile.handle_key(key(KeyCode::Char('n')), &results),
            ProfileAction::None
        );
        assert!(!profile.confirm_reset);
        profile.handle_key(key(KeyCode::Char('R')), &results);
        assert_eq!(
            profile.handle_key(key(KeyCode::Char('y')), &results),
            ProfileAction::Reset
        );
        assert_eq!(
            profile.handle_key(key(KeyCode::Esc), &results),
            ProfileAction::Back
        );
    }

    #[test]
    fn streak_of_days() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap();
        let mut days = BTreeMap::new();
        assert_eq!(current_streak(&days, today), 0);
        days.insert(today - Duration::days(1), 1000);
        days.insert(today - Duration::days(2), 1000);
        days.insert(today - Duration::days(4), 1000);
        assert_eq!(current_streak(&days, today), 2);
        days.insert(today, 1000);
        assert_eq!(current_streak(&days, today), 3);
    }

    #[test]
    fn layouts_by_use() {
        let r = |layout: &str| {
            let mut r = crate::result::tests::result(0, 300.0, 1.0, &[('a', 200)]);
            r.layout = layout.into();
            r
        };
        let results = vec![r("en-dvorak"), r("en-us"), r("en-us")];
        assert_eq!(layouts_in(&results), vec!["en-us", "en-dvorak"]);
    }
}
