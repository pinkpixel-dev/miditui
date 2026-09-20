use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

use super::theme::Theme;
use crate::app::App;

pub struct TransportWidget<'a> {
    pub app: &'a mut App,
    pub theme: &'a Theme,
}

impl<'a> Widget for TransportWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        // Fill background
        let base_style = Style::default().bg(self.theme.surface).fg(self.theme.text);
        for x in area.x..area.x + area.width {
            for y in area.y..area.y + area.height {
                buf[(x, y)].set_char(' ').set_style(base_style);
            }
        }

        let current_time = self.app.get_current_time_secs();
        let total_time = self.app.song.duration_secs;
        let is_playing = self.app.is_playing();
        let (measure, beat) = self.app.song.time_to_measure_beat(current_time);

        // Format time strings (MM:SS.S)
        let cur_m = (current_time / 60.0).floor() as u32;
        let cur_s = current_time % 60.0;
        let tot_m = (total_time / 60.0).floor() as u32;
        let tot_s = total_time % 60.0;

        let play_icon = if is_playing { "⏸" } else { "⏵" };
        let play_icon_style = Style::default()
            .fg(if is_playing { self.theme.accent } else { self.theme.text })
            .add_modifier(Modifier::BOLD);

        // Left info block: [⏵] [⏹] 0:23.1 / 2:50.0  Bar 12.3  128 BPM
        let mut x = area.x + 1;
        let y = area.y;

        // Play/Pause button click zone
        self.app.play_button_rect = Some(Rect { x, y, width: 3, height: 1 });
        buf.set_string(x, y, format!(" {} ", play_icon), play_icon_style);
        x += 4;

        // Stop button
        buf.set_string(x, y, "⏹", Style::default().fg(self.theme.text_muted));
        x += 3;

        // Timecode
        let time_str = format!("{:02}:{:04.1} / {:02}:{:04.1}", cur_m, cur_s, tot_m, tot_s);
        buf.set_string(x, y, time_str, Style::default().fg(self.theme.text).add_modifier(Modifier::BOLD));
        x += 23;

        // Bar / Beat
        let bar_str = format!("Bar {:<2}.{:<1}", measure, (beat * 10.0).round() as u32 % 10);
        buf.set_string(x, y, bar_str, Style::default().fg(self.theme.text_muted));
        x += 12;

        // BPM
        let bpm_str = format!("{:.0} BPM", self.app.song.initial_bpm);
        buf.set_string(x, y, bpm_str, Style::default().fg(self.theme.text_subtle));
        x += 10;

        // Right side info: Volume + Loop
        let vol = self.app.get_volume();
        let loop_text = if self.app.loop_playback { "Loop: ON" } else { "Loop: OFF" };
        let right_info = format!("{}  Vol: {:>3}%", loop_text, vol);
        let right_width = right_info.len() as u16 + 2;
        let right_x = (area.x + area.width).saturating_sub(right_width);

        if right_x > x + 15 {
            buf.set_string(
                right_x,
                y,
                right_info,
                Style::default().fg(self.theme.text_muted),
            );
        }

        // Center: Interactive Progress / Scrubber Bar
        let scrub_start_x = x + 2;
        let scrub_end_x = right_x.saturating_sub(2);

        if scrub_end_x > scrub_start_x + 8 {
            let scrub_width = scrub_end_x - scrub_start_x;
            self.app.scrub_bar_rect = Some(Rect {
                x: scrub_start_x,
                y,
                width: scrub_width,
                height: 1,
            });

            let progress = if total_time > 0.0 {
                (current_time / total_time).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let knob_pos = (progress * (scrub_width.saturating_sub(1) as f64)).round() as u16;

            for i in 0..scrub_width {
                let current_bar_x = scrub_start_x + i;
                let cell = &mut buf[(current_bar_x, y)];
                if i < knob_pos {
                    cell.set_char('━')
                        .set_style(Style::default().bg(self.theme.surface).fg(self.theme.accent));
                } else if i == knob_pos {
                    cell.set_char('●')
                        .set_style(Style::default().bg(self.theme.surface).fg(Color::Rgb(255, 255, 255)).add_modifier(Modifier::BOLD));
                } else {
                    cell.set_char('─')
                        .set_style(Style::default().bg(self.theme.surface).fg(Color::Rgb(60, 60, 70)));
                }
            }
        }
    }
}
