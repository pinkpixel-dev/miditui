use ratatui::layout::Rect;

use crate::audio::AudioEngine;
use crate::midi::Song;

pub struct App {
    pub song: Song,
    pub audio: Option<AudioEngine>,
    pub view_time_start: f64,
    pub zoom: f64,
    pub view_max_pitch: u8,
    pub selected_track_idx: usize,
    pub follow_playhead: bool,
    pub loop_playback: bool,
    pub show_help: bool,
    pub should_quit: bool,
    pub using_soundfont: bool,
    pub soundfont_name: Option<String>,

    // Hit-testing bounding boxes for mouse & touch interaction
    pub track_click_zones: Vec<(Rect, usize)>,
    pub mute_click_zones: Vec<(Rect, usize)>,
    pub solo_click_zones: Vec<(Rect, usize)>,
    pub play_button_rect: Option<Rect>,
    pub scrub_bar_rect: Option<Rect>,
}

impl App {
    pub fn new(song: Song, audio: Option<AudioEngine>) -> Self {
        // Calculate average or median pitch to center the initial viewport
        let mut sum_pitch: u64 = 0;
        let mut note_count: u64 = 0;
        for track in &song.tracks {
            for note in &track.notes {
                sum_pitch += note.pitch as u64;
                note_count += 1;
            }
        }

        let center_pitch = sum_pitch
            .checked_div(note_count)
            .map(|p| p as u8)
            .unwrap_or(60);

        let view_max_pitch = (center_pitch + 14).clamp(36, 127);

        let using_soundfont = audio.as_ref().map(|a| a.using_soundfont).unwrap_or(false);
        let soundfont_name = audio.as_ref().and_then(|a| a.soundfont_name.clone());

        Self {
            song,
            audio,
            view_time_start: 0.0,
            zoom: 1.0,
            view_max_pitch,
            selected_track_idx: 0,
            follow_playhead: true,
            loop_playback: false,
            show_help: false,
            should_quit: false,
            using_soundfont,
            soundfont_name,
            track_click_zones: Vec::new(),
            mute_click_zones: Vec::new(),
            solo_click_zones: Vec::new(),
            play_button_rect: None,
            scrub_bar_rect: None,
        }
    }

    /// Visible time duration in seconds across `columns` terminal cells
    pub fn viewport_duration(&self, columns: u16) -> f64 {
        let cols = columns.max(1) as f64;
        // Base: 4 columns = 1 second at zoom 1.0
        (cols / (4.0 * self.zoom)).max(0.5)
    }

    pub fn get_current_time_secs(&self) -> f64 {
        if let Some(audio) = &self.audio {
            audio.get_current_time_secs()
        } else {
            0.0
        }
    }

    pub fn is_playing(&self) -> bool {
        self.audio.as_ref().map(|a| a.is_playing()).unwrap_or(false)
    }

    pub fn get_volume(&self) -> u8 {
        self.audio.as_ref().map(|a| a.get_volume()).unwrap_or(85)
    }

    pub fn toggle_play(&mut self) {
        if let Some(audio) = &self.audio {
            audio.toggle_play();
        }
    }

    pub fn seek(&mut self, target_secs: f64) {
        let clamped = target_secs.clamp(0.0, self.song.duration_secs);
        if let Some(audio) = &self.audio {
            audio.seek(clamped);
        }
        if self.follow_playhead {
            self.view_time_start = (clamped - 2.0).max(0.0);
        }
    }

    pub fn seek_relative(&mut self, delta_secs: f64) {
        let current = self.get_current_time_secs();
        self.seek(current + delta_secs);
    }

    pub fn seek_measure(&mut self, delta_measures: i32) {
        let current = self.get_current_time_secs();
        let (cur_m, _) = self.song.time_to_measure_beat(current);
        let target_m = ((cur_m as i32 + delta_measures).max(1)) as u32;
        let measure_sec = (60.0 / self.song.initial_bpm) * 4.0;
        let target_time = (target_m - 1) as f64 * measure_sec;
        self.seek(target_time);
    }

    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 1.25).min(8.0);
    }

    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / 1.25).max(0.2);
    }

    pub fn scroll_pitch(&mut self, delta: i8) {
        let new_pitch = (self.view_max_pitch as i16 + delta as i16).clamp(24, 127);
        self.view_max_pitch = new_pitch as u8;
    }

    pub fn select_next_track(&mut self) {
        if !self.song.tracks.is_empty() {
            self.selected_track_idx = (self.selected_track_idx + 1) % self.song.tracks.len();
        }
    }

    pub fn select_prev_track(&mut self) {
        if !self.song.tracks.is_empty() {
            if self.selected_track_idx == 0 {
                self.selected_track_idx = self.song.tracks.len() - 1;
            } else {
                self.selected_track_idx -= 1;
            }
        }
    }

    pub fn toggle_mute_selected(&mut self) {
        let idx = self.selected_track_idx;
        self.toggle_mute_track(idx);
    }

    pub fn toggle_solo_selected(&mut self) {
        let idx = self.selected_track_idx;
        self.toggle_solo_track(idx);
    }

    pub fn toggle_mute_track(&mut self, idx: usize) {
        if let Some(track) = self.song.tracks.get_mut(idx) {
            track.muted = !track.muted;
            let muted = track.muted;
            if let Some(audio) = &self.audio {
                audio.set_track_mute(idx, muted);
            }
        }
    }

    pub fn toggle_solo_track(&mut self, idx: usize) {
        if let Some(track) = self.song.tracks.get_mut(idx) {
            track.soloed = !track.soloed;
            let soloed = track.soloed;
            if let Some(audio) = &self.audio {
                audio.set_track_solo(idx, soloed);
            }
        }
    }

    pub fn adjust_volume(&mut self, delta: i8) {
        let current = self.get_volume() as i16;
        let new_vol = (current + delta as i16).clamp(0, 100) as u8;
        if let Some(audio) = &self.audio {
            audio.set_volume(new_vol);
        }
    }

    /// Automatically pan the viewport when playhead reaches the right edge
    pub fn update_auto_follow(&mut self, grid_width: u16) {
        if !self.follow_playhead {
            return;
        }

        let current_time = self.get_current_time_secs();
        let view_duration = self.viewport_duration(grid_width);

        // When playhead moves past 75% of viewport width, shift viewport
        if current_time >= self.view_time_start + (view_duration * 0.75) {
            self.view_time_start = (current_time - (view_duration * 0.25)).max(0.0);
        } else if current_time < self.view_time_start {
            self.view_time_start = current_time.max(0.0);
        }
    }

    /// Retrieve pitches sounding at timestamp `time_secs`
    pub fn get_active_pitches(&self, time_secs: f64) -> Vec<u8> {
        let mut active = Vec::new();
        let has_solo = self.song.tracks.iter().any(|t| t.soloed);

        for track in &self.song.tracks {
            let is_active_track = if has_solo {
                track.soloed
            } else {
                !track.muted
            };
            if !is_active_track {
                continue;
            }
            for note in &track.notes {
                if time_secs >= note.start_secs && time_secs <= note.end_secs {
                    active.push(note.pitch);
                }
            }
        }
        active
    }

    /// Process mouse click at terminal coordinates (x, y)
    pub fn handle_mouse_click(&mut self, x: u16, y: u16) {
        // 1. Check scrubber progress bar
        if let Some(rect) = self.scrub_bar_rect {
            if y == rect.y && x >= rect.x && x < rect.x + rect.width {
                let rel_x = (x - rect.x) as f64;
                let ratio = rel_x / (rect.width.max(1) as f64);
                let target_secs = ratio * self.song.duration_secs;
                self.seek(target_secs);
                return;
            }
        }

        // 2. Check play button
        if let Some(rect) = self.play_button_rect {
            if y == rect.y && x >= rect.x && x < rect.x + rect.width {
                self.toggle_play();
                return;
            }
        }

        // 3. Check track mute buttons
        for &(rect, idx) in &self.mute_click_zones {
            if y == rect.y && x >= rect.x && x < rect.x + rect.width {
                self.toggle_mute_track(idx);
                return;
            }
        }

        // 4. Check track solo buttons
        for &(rect, idx) in &self.solo_click_zones {
            if y == rect.y && x >= rect.x && x < rect.x + rect.width {
                self.toggle_solo_track(idx);
                return;
            }
        }

        // 5. Check track card selection
        for &(rect, idx) in &self.track_click_zones {
            if x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height {
                self.selected_track_idx = idx;
                return;
            }
        }
    }
}
