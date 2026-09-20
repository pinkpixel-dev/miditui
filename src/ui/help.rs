use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use super::theme::Theme;

pub fn render_help_modal(f: &mut Frame, theme: &Theme, area: Rect) {
    let modal_width = 62.min(area.width.saturating_sub(4));
    let modal_height = 22.min(area.height.saturating_sub(2));

    let modal_x = (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect {
        x: modal_x,
        y: modal_y,
        width: modal_width,
        height: modal_height,
    };

    f.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent))
        .title(" Keyboard & Mouse Shortcuts [Esc / ? to close] ")
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(Color::Rgb(24, 24, 28)).fg(theme.text));

    let inner = block.inner(modal_area);
    f.render_widget(block, modal_area);

    let shortcuts = [
        ("Space", "Play / Pause playback"),
        ("Left / Right (h / l)", "Seek backward / forward 5s"),
        ("[ / ]", "Seek backward / forward 1 measure"),
        ("Home", "Rewind to beginning"),
        ("Up / Down (k / j)", "Scroll pitch view up / down"),
        ("+ / -", "Zoom time axis in / out"),
        ("f", "Toggle auto-follow playhead"),
        ("r", "Toggle loop playback"),
        ("Tab / Shift+Tab", "Select next / previous track"),
        ("m", "Toggle mute on selected track"),
        ("s", "Toggle solo on selected track"),
        ("1 .. 9", "Toggle mute for track 1 to 9"),
        (", / . (< / >)", "Decrease / increase volume"),
        ("Mouse Click", "Click track, [M]/[S], play button, or scrub bar"),
        ("Mouse Scroll", "Scroll pitch or pan timeline"),
        ("q / Esc", "Close help modal / Quit application"),
    ];

    let mut lines = Vec::new();
    lines.push(Line::from(""));

    for (key, desc) in shortcuts {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:<22}", key), Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled(desc, Style::default().fg(theme.text_muted)),
        ]));
    }

    f.render_widget(Paragraph::new(lines), inner);
}
