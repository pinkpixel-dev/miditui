use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use super::theme::Theme;
use crate::app::App;

pub fn render_header(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(theme.border))
        .style(theme.surface_style());

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Left spans: App badge + Song Title
    let title = if app.song.title.is_empty() {
        "Untitled Track"
    } else {
        &app.song.title
    };

    let synth_badge = if app.using_soundfont {
        let name = app.soundfont_name.as_deref().unwrap_or("SoundFont");
        Span::styled(format!(" [SF2: {}] ", name), Style::default().fg(theme.success))
    } else {
        Span::styled(" [Synth: Fallback Wave] ", Style::default().fg(theme.warning))
    };

    let left_spans = vec![
        Span::styled(" MIDITUI ", Style::default().bg(theme.accent).fg(Color::Rgb(18, 18, 20)).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(title, theme.header_title()),
        Span::raw("  "),
        Span::styled(
            format!("{} tracks", app.song.tracks.len()),
            Style::default().fg(theme.text_subtle),
        ),
        Span::raw(" "),
        synth_badge,
    ];

    // Right spans: Zoom, Follow mode, Help
    let follow_style = if app.follow_playhead {
        Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_subtle)
    };

    let right_spans = vec![
        Span::styled("Zoom: ", Style::default().fg(theme.text_subtle)),
        Span::styled(format!("{:.1}x ", app.zoom), Style::default().fg(theme.text)),
        Span::styled("Snap: ", Style::default().fg(theme.text_subtle)),
        Span::styled("1/16 ", Style::default().fg(theme.text)),
        Span::styled("Follow [F]: ", Style::default().fg(theme.text_subtle)),
        Span::styled(if app.follow_playhead { "ON " } else { "OFF " }, follow_style),
        Span::styled("[?] Help ", Style::default().fg(theme.text_muted).add_modifier(Modifier::BOLD)),
    ];

    f.render_widget(
        Paragraph::new(Line::from(left_spans)).alignment(Alignment::Left),
        inner,
    );

    f.render_widget(
        Paragraph::new(Line::from(right_spans)).alignment(Alignment::Right),
        inner,
    );
}
