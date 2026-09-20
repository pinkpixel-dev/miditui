use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use super::theme::Theme;
use crate::app::App;

pub fn render_track_list(f: &mut Frame, app: &mut App, theme: &Theme, area: Rect) {
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(theme.border))
        .style(theme.surface_style());

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    // Header line inside sidebar
    let header_line = Line::from(vec![
        Span::styled(" TRACKS ", Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("({})", app.song.tracks.len()),
            Style::default().fg(theme.text_subtle),
        ),
    ]);
    f.render_widget(
        Paragraph::new(header_line).alignment(Alignment::Left),
        Rect { x: inner.x, y: inner.y, width: inner.width, height: 1 },
    );

    // Clear and record hit testing zones
    app.track_click_zones.clear();
    app.mute_click_zones.clear();
    app.solo_click_zones.clear();

    let mut current_y = inner.y + 2;
    let max_y = inner.y + inner.height;

    for (idx, track) in app.song.tracks.iter().enumerate() {
        if current_y + 2 >= max_y {
            break;
        }

        let is_selected = idx == app.selected_track_idx;
        let bg_color = if is_selected {
            theme.surface_hover
        } else {
            theme.surface
        };

        let card_height = if inner.height > 25 { 3 } else { 2 };
        let card_area = Rect {
            x: inner.x,
            y: current_y,
            width: inner.width,
            height: card_height,
        };

        app.track_click_zones.push((card_area, idx));

        // Line 1: Swatch, Name, Channel
        let name_max_len = (inner.width as usize).saturating_sub(9);
        let truncated_name = if track.name.len() > name_max_len {
            format!("{}…", &track.name[..name_max_len.saturating_sub(1)])
        } else {
            track.name.clone()
        };

        let select_marker = if is_selected { "▶ " } else { "  " };

        let line1 = Line::from(vec![
            Span::styled(select_marker, Style::default().fg(theme.accent)),
            Span::styled("● ", Style::default().fg(track.color)),
            Span::styled(
                truncated_name,
                Style::default()
                    .fg(if is_selected { theme.text } else { theme.text_muted })
                    .add_modifier(if is_selected { Modifier::BOLD } else { Modifier::empty() }),
            ),
            Span::styled(
                format!(" ch{}", track.channel + 1),
                Style::default().fg(theme.text_subtle),
            ),
        ]);

        f.render_widget(
            Paragraph::new(line1).style(Style::default().bg(bg_color)),
            Rect { x: card_area.x, y: card_area.y, width: card_area.width, height: 1 },
        );

        // Line 2: Instrument name (if 3 lines) or Mute/Solo buttons
        if card_height >= 3 {
            let inst_max_len = (inner.width as usize).saturating_sub(6);
            let truncated_inst = if track.instrument_name.len() > inst_max_len {
                format!("{}…", &track.instrument_name[..inst_max_len.saturating_sub(1)])
            } else {
                track.instrument_name.clone()
            };

            let line2 = Line::from(vec![
                Span::raw("    "),
                Span::styled(truncated_inst, Style::default().fg(theme.text_subtle)),
            ]);

            f.render_widget(
                Paragraph::new(line2).style(Style::default().bg(bg_color)),
                Rect { x: card_area.x, y: card_area.y + 1, width: card_area.width, height: 1 },
            );
        }

        // Controls Line: [M] [S] Vol: ||||....
        let controls_y = card_area.y + card_height - 1;
        let mute_style = if track.muted {
            Style::default().bg(Color::Rgb(239, 68, 68)).fg(Color::Rgb(255, 255, 255)).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_subtle)
        };

        let solo_style = if track.soloed {
            Style::default().bg(Color::Rgb(234, 179, 8)).fg(Color::Rgb(18, 18, 20)).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_subtle)
        };

        let mute_rect = Rect { x: card_area.x + 4, y: controls_y, width: 3, height: 1 };
        let solo_rect = Rect { x: card_area.x + 8, y: controls_y, width: 3, height: 1 };
        app.mute_click_zones.push((mute_rect, idx));
        app.solo_click_zones.push((solo_rect, idx));

        let vol_bars = "▮▮▮▮▯▯▯";

        let controls_line = Line::from(vec![
            Span::raw("    "),
            Span::styled("[M]", mute_style),
            Span::raw(" "),
            Span::styled("[S]", solo_style),
            Span::raw(" "),
            Span::styled(vol_bars, Style::default().fg(theme.text_subtle)),
        ]);

        f.render_widget(
            Paragraph::new(controls_line).style(Style::default().bg(bg_color)),
            Rect { x: card_area.x, y: controls_y, width: card_area.width, height: 1 },
        );

        current_y += card_height + 1;
    }
}
