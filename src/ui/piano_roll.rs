use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

use super::theme::Theme;
use crate::app::App;
use crate::midi::song::Song;

pub struct PianoRollWidget<'a> {
    pub app: &'a App,
    pub theme: &'a Theme,
}

impl<'a> Widget for PianoRollWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 12 || area.height < 4 {
            return;
        }

        let key_width: u16 = 6;
        let ruler_height: u16 = 1;

        let piano_area = Rect {
            x: area.x,
            y: area.y + ruler_height,
            width: key_width,
            height: area.height.saturating_sub(ruler_height),
        };

        let ruler_area = Rect {
            x: area.x + key_width,
            y: area.y,
            width: area.width.saturating_sub(key_width),
            height: ruler_height,
        };

        let grid_area = Rect {
            x: area.x + key_width,
            y: area.y + ruler_height,
            width: area.width.saturating_sub(key_width),
            height: area.height.saturating_sub(ruler_height),
        };

        let current_time = self.app.get_current_time_secs();
        let active_pitches = self.app.get_active_pitches(current_time);

        // 1. Render Piano Keys Column
        self.render_piano_keys(piano_area, buf, &active_pitches);

        // 2. Render Timeline Ruler
        self.render_ruler(ruler_area, buf);

        // 3. Render Note Grid and Playhead
        self.render_grid(grid_area, buf, current_time);
    }
}

impl<'a> PianoRollWidget<'a> {
    fn render_piano_keys(&self, area: Rect, buf: &mut Buffer, active_pitches: &[u8]) {
        let max_pitch = self.app.view_max_pitch;

        for row in 0..area.height {
            let pitch = max_pitch.saturating_sub(row as u8);
            let y = area.y + row;
            let is_black = Song::is_black_key(pitch);
            let is_active = active_pitches.contains(&pitch);
            let note_name = Song::pitch_to_name(pitch);

            let (bg_color, fg_color) = if is_active {
                (self.theme.active_key, Color::Rgb(18, 18, 20))
            } else if is_black {
                (self.theme.black_key_bg, self.theme.black_key_fg)
            } else {
                (self.theme.white_key_bg, self.theme.white_key_fg)
            };

            let key_str = format!("{:>4} ", note_name);
            let key_style = Style::default()
                .bg(bg_color)
                .fg(fg_color)
                .add_modifier(if is_active || pitch.is_multiple_of(12) { Modifier::BOLD } else { Modifier::empty() });

            for (col, ch) in key_str.chars().take(area.width as usize).enumerate() {
                let cell = &mut buf[(area.x + col as u16, y)];
                cell.set_char(ch).set_style(key_style);
            }
        }
    }

    fn render_ruler(&self, area: Rect, buf: &mut Buffer) {
        let ruler_style = Style::default()
            .bg(self.theme.surface)
            .fg(self.theme.text_subtle);

        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_char('─').set_style(ruler_style);
        }

        let time_start = self.app.view_time_start;
        let time_end = time_start + self.app.viewport_duration(area.width);
        let sec_per_col = self.app.viewport_duration(area.width) / (area.width as f64).max(1.0);

        let bpm = self.app.song.initial_bpm;
        let beat_duration = 60.0 / bpm;
        let measure_duration = beat_duration * 4.0;

        let start_measure = (time_start / measure_duration).floor() as i64;
        let end_measure = (time_end / measure_duration).ceil() as i64;

        for m in start_measure..=end_measure {
            let m_time = m as f64 * measure_duration;
            if m_time >= time_start && m_time <= time_end {
                let col = ((m_time - time_start) / sec_per_col).round() as u16;
                if col < area.width {
                    let label = format!("Bar {}", m + 1);
                    let label_style = Style::default()
                        .bg(self.theme.surface)
                        .fg(self.theme.text)
                        .add_modifier(Modifier::BOLD);

                    for (idx, ch) in label.chars().enumerate() {
                        let target_x = area.x + col + idx as u16;
                        if target_x < area.x + area.width {
                            buf[(target_x, area.y)]
                                .set_char(ch)
                                .set_style(label_style);
                        }
                    }
                }
            }
        }
    }

    fn render_grid(&self, area: Rect, buf: &mut Buffer, current_time: f64) {
        let view_time_start = self.app.view_time_start;
        let view_duration = self.app.viewport_duration(area.width);
        let view_time_end = view_time_start + view_duration;
        let sec_per_col = view_duration / (area.width as f64).max(1.0);

        let bpm = self.app.song.initial_bpm;
        let beat_duration = 60.0 / bpm;
        let measure_duration = beat_duration * 4.0;

        let max_pitch = self.app.view_max_pitch;
        let min_pitch = max_pitch.saturating_sub(area.height as u8);

        // 1. Draw grid background with measure/beat lines
        for y_offset in 0..area.height {
            let y = area.y + y_offset;
            let pitch = max_pitch.saturating_sub(y_offset as u8);
            let is_black = Song::is_black_key(pitch);

            let row_bg = if is_black {
                Color::Rgb(15, 15, 17)
            } else {
                Color::Rgb(20, 20, 23)
            };

            for x_offset in 0..area.width {
                let x = area.x + x_offset;
                let t = view_time_start + (x_offset as f64 * sec_per_col);
                let measure_rel = (t % measure_duration) / sec_per_col;
                let beat_rel = (t % beat_duration) / sec_per_col;

                let (ch, fg) = if measure_rel < 1.0 {
                    ('│', Color::Rgb(45, 45, 55))
                } else if beat_rel < 1.0 {
                    ('·', Color::Rgb(35, 35, 42))
                } else {
                    (' ', Color::Rgb(25, 25, 30))
                };

                let cell = &mut buf[(x, y)];
                cell.set_char(ch).set_fg(fg).set_bg(row_bg);
            }
        }

        // 2. Render notes for each track
        let has_solo = self.app.song.tracks.iter().any(|t| t.soloed);

        for track in &self.app.song.tracks {
            // Check if track is muted or silenced by solo
            let is_active_track = if has_solo {
                track.soloed
            } else {
                !track.muted
            };

            let note_color = if is_active_track {
                track.color
            } else {
                Color::Rgb(50, 50, 55)
            };

            for note in &track.notes {
                // Skip notes out of viewport
                if note.end_secs < view_time_start || note.start_secs > view_time_end {
                    continue;
                }
                if note.pitch > max_pitch || note.pitch < min_pitch {
                    continue;
                }

                let row = (max_pitch - note.pitch) as u16;
                if row >= area.height {
                    continue;
                }
                let y = area.y + row;

                let start_col = if note.start_secs <= view_time_start {
                    0
                } else {
                    ((note.start_secs - view_time_start) / sec_per_col).floor() as u16
                };

                let end_col = if note.end_secs >= view_time_end {
                    area.width
                } else {
                    ((note.end_secs - view_time_start) / sec_per_col).ceil() as u16
                };

                let note_is_playing = current_time >= note.start_secs && current_time <= note.end_secs;

                let span_style = if note_is_playing {
                    Style::default()
                        .bg(note_color)
                        .fg(Color::Rgb(255, 255, 255))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().bg(note_color).fg(note_color)
                };

                for col in start_col..end_col {
                    if col >= area.width {
                        break;
                    }
                    let x = area.x + col;
                    let cell = &mut buf[(x, y)];

                    // Caps for start and end
                    if col == start_col && col + 1 == end_col {
                        cell.set_char('█').set_style(span_style);
                    } else if col == start_col {
                        cell.set_char('▌').set_style(span_style);
                    } else if col == end_col - 1 {
                        cell.set_char('▐').set_style(span_style);
                    } else {
                        cell.set_char('█').set_style(span_style);
                    }
                }
            }
        }

        // 3. Render playhead line
        if current_time >= view_time_start && current_time <= view_time_end {
            let playhead_col = ((current_time - view_time_start) / sec_per_col).round() as u16;
            if playhead_col < area.width {
                let x = area.x + playhead_col;
                let playhead_style = Style::default()
                    .fg(self.theme.playhead)
                    .add_modifier(Modifier::BOLD);
                for row in 0..area.height {
                    let y = area.y + row;
                    buf[(x, y)].set_char('│').set_style(playhead_style);
                }
            }
        }
    }
}
