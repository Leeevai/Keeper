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
