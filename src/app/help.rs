//! The help screen.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};

use super::{App, RenderContext};

pub fn render(frame: &mut Frame, area: Rect, ctx: &RenderContext, app: &App) {
    let s = ctx.styles;
    let h = |t: &str| Line::from(Span::styled(t.to_string(), s.title));
    let p = |t: &str| Line::from(Span::styled(t.to_string(), s.text));
    let k = |keys: &str, what: &str| {
        Line::from(vec![
            Span::styled(format!("  {keys:<22}"), s.value),
            Span::styled(what.to_string(), s.name),
        ])
    };
    let target = ctx
        .settings
        .speed_unit
        .format(ctx.settings.target_speed as f64);
    let lines = vec![
        h("How it works"),
        p(
            "Keeper teaches touch typing the way keybr.com does. Guided lessons start with the six most \
           frequent letters of your language and generate pronounceable words from them. Every key \
           has its own speed, measured from how long you take to press it.",
        ),
        p(""),
        p(&format!(
            "Once all the letters reach the target speed ({target}), a new letter is unlocked. The letter \
             you are slowest at is the current key: it is underlined in the key list and every word \
             contains it. Keys are colored from red (slow) to green (at the target speed)."
        )),
        p(""),
        p(
            "Type the text as it appears, there is no need to press Enter. By default the cursor stops on \
           a wrong key until you press the right one, and the key is counted as an error.",
        ),
        p(""),
        h("Metrics"),
        p(
            "Speed is measured in words per minute, a word being five characters (change the unit in \
           Settings). Accuracy is the share of characters typed without errors. The score grows with \
           speed, accuracy, length and the number of distinct letters. The numbers in brackets compare \
           the last lesson with your average.",
        ),
        p(""),
        h("Keys"),
        k("Tab / Shift+Tab", "next / previous screen"),
        k(
            &format!("F1 … F{}", super::Screen::ALL.len()),
            "go to a screen",
        ),
        k(
            "Esc",
            "restart the lesson (practice), go back (other screens)",
        ),
        k("Ctrl+N or Ctrl+→", "skip to a new lesson"),
        k(
            "Ctrl+V",
            "switch between the normal, compact and bare views",
        ),
        k("Backspace", "delete a wrong character"),
        k("Ctrl+W, Alt+Backspace", "delete the current word"),
        k("Ctrl+Q or Ctrl+C", "quit"),
        p(""),
        h("Your data"),
        p(&format!(
            "Settings and lesson results are saved in {}. Results are kept in results.jsonl, one lesson \
             per line, in the same format keybr.com uses when you download your data, so you can import \
             your keybr.com history with `keeper import <file>`.",
            app.storage.dir().display()
        )),
        p(""),
        h("Credits"),
        p(
            "Keeper is a terminal port of keybr.com by Aliaksandr Radzivanovich (github.com/aradzie/keybr.com). \
           It uses keybr's algorithms, language models, word lists, books and keyboard layouts under the \
           GNU AGPL v3.",
        ),
    ];
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(s.border)
        .title(Span::styled(" Help ", s.title));
    let area = crate::ui::centered(area, 90, area.height);
    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: true })
        .scroll((app.help_scroll, 0));
    frame.render_widget(paragraph, area);
}
