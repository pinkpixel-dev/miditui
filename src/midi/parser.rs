use std::collections::HashMap;
use std::path::Path;
use anyhow::{Context, Result};
use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use ratatui::style::Color;

use super::song::{
    gm_instrument_name, Note, PlaybackEvent, PlaybackEventKind, Song, TempoMarker,
    TimeSignatureMarker, Track,
};

/// 16 distinct, stylish 24-bit RGB track colors
pub const TRACK_PALETTE: [Color; 16] = [
    Color::Rgb(64, 169, 255),  // Electric Sky Blue
    Color::Rgb(247, 84, 140),  // Vivid Rose Pink
    Color::Rgb(82, 196, 26),   // Fresh Lime Green
    Color::Rgb(250, 173, 20),  // Warm Golden Amber
    Color::Rgb(146, 84, 222),  // Deep Violet
    Color::Rgb(19, 194, 194),  // Turquoise Cyan
    Color::Rgb(255, 122, 69),  // Coral Orange
    Color::Rgb(255, 192, 203), // Pastel Blush
    Color::Rgb(54, 207, 201),  // Mint Green
    Color::Rgb(179, 127, 235), // Lavender
    Color::Rgb(255, 77, 79),   // Crimson Red
    Color::Rgb(114, 46, 209),  // Royal Purple
    Color::Rgb(250, 219, 20),  // Lemon Yellow
    Color::Rgb(47, 84, 235),   // Cobalt Blue
    Color::Rgb(235, 47, 150),  // Hot Magenta
    Color::Rgb(89, 126, 247),  // Periwinkle
];

pub struct MidiParser;

impl MidiParser {
    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<Song> {
        let bytes = std::fs::read(path.as_ref())
            .with_context(|| format!("Failed to read MIDI file at {:?}", path.as_ref()))?;
        let title = path
            .as_ref()
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();
        Self::parse_bytes(&bytes, title)
    }

    pub fn parse_bytes(bytes: &[u8], default_title: String) -> Result<Song> {
        let smf = Smf::parse(bytes).context("Failed to parse MIDI file format")?;

        let ticks_per_beat = match smf.header.timing {
            Timing::Metrical(tpb) => tpb.as_int(),
            Timing::Timecode(fps, subframe) => (fps.as_f32() * subframe as f32).round() as u16,
        };
        let ticks_per_beat = if ticks_per_beat == 0 { 480 } else { ticks_per_beat };

        // Step 1: Gather all tempo and time signature events across all tracks
        let mut raw_tempo_events = Vec::new();
        let mut raw_time_sigs = Vec::new();
        let mut song_title = default_title;

        for track in &smf.tracks {
            let mut current_tick: u64 = 0;
            for event in track {
                current_tick += event.delta.as_int() as u64;
                if let TrackEventKind::Meta(meta) = event.kind {
                    match meta {
                        MetaMessage::Tempo(tempo) => {
                            raw_tempo_events.push((current_tick, tempo.as_int()));
                        }
                        MetaMessage::TimeSignature(num, den, _, _) => {
                            raw_time_sigs.push((current_tick, num, 2_u8.pow(den as u32)));
                        }
                        MetaMessage::TrackName(name_bytes) => {
                            if let Ok(name) = std::str::from_utf8(name_bytes) {
                                let trimmed = name.trim();
                                if !trimmed.is_empty() && song_title == "Untitled" {
                                    song_title = trimmed.to_string();
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Sort tempo changes by tick
        raw_tempo_events.sort_by_key(|&(tick, _)| tick);
        raw_time_sigs.sort_by_key(|&(tick, _, _)| tick);

        // Build continuous Tempo Markers with precise cumulative seconds
        let mut tempo_markers = Vec::new();
        let mut current_tick: u64 = 0;
        let mut current_time_secs: f64 = 0.0;
        let mut current_micros_per_beat: u32 = 500_000; // default 120 BPM

        if raw_tempo_events.is_empty() || raw_tempo_events[0].0 > 0 {
            tempo_markers.push(TempoMarker {
                tick: 0,
                timestamp_secs: 0.0,
                bpm: 60_000_000.0 / current_micros_per_beat as f64,
                micros_per_beat: current_micros_per_beat,
            });
        }

        for (target_tick, new_micros) in raw_tempo_events {
            if target_tick > current_tick {
                let delta_ticks = target_tick - current_tick;
                let secs_per_tick = (current_micros_per_beat as f64 / 1_000_000.0) / ticks_per_beat as f64;
                current_time_secs += delta_ticks as f64 * secs_per_tick;
                current_tick = target_tick;
            }
            current_micros_per_beat = new_micros;
            let bpm = 60_000_000.0 / new_micros as f64;
            tempo_markers.push(TempoMarker {
                tick: target_tick,
                timestamp_secs: current_time_secs,
                bpm,
                micros_per_beat: new_micros,
            });
        }

        // Build time signatures
        let mut time_signatures = Vec::new();
        if raw_time_sigs.is_empty() || raw_time_sigs[0].0 > 0 {
            time_signatures.push(TimeSignatureMarker {
                tick: 0,
                timestamp_secs: 0.0,
                numerator: 4,
                denominator: 4,
            });
        }
        for (tick, num, den) in raw_time_sigs {
            let t = tick_to_secs(tick, ticks_per_beat, &tempo_markers);
            time_signatures.push(TimeSignatureMarker {
                tick,
                timestamp_secs: t,
                numerator: num,
                denominator: den,
            });
        }

        // Step 2: Parse tracks and note spans
        let mut parsed_tracks = Vec::new();
        let mut all_playback_events = Vec::new();
        let mut total_duration_secs: f64 = 0.0;
        let mut total_ticks: u64 = 0;

        for (track_idx, raw_track) in smf.tracks.iter().enumerate() {
            let mut track_tick: u64 = 0;
            let mut track_name = format!("Track {}", track_idx + 1);
            let mut program: u8 = 0;
            let mut channel: u8 = (track_idx % 16) as u8;
            let mut active_notes: HashMap<(u8, u8), (u64, f64, u8)> = HashMap::new(); // (channel, pitch) -> (start_tick, start_secs, velocity)
            let mut track_notes = Vec::new();

            for event in raw_track {
                track_tick += event.delta.as_int() as u64;
                let current_time = tick_to_secs(track_tick, ticks_per_beat, &tempo_markers);

                match event.kind {
                    TrackEventKind::Meta(MetaMessage::TrackName(name_bytes)) => {
                        if let Ok(name) = std::str::from_utf8(name_bytes) {
                            let trimmed = name.trim();
                            if !trimmed.is_empty() {
                                track_name = trimmed.to_string();
                            }
                        }
                    }
                    TrackEventKind::Midi { channel: ch, message } => {
                        let ch_u8 = ch.as_int();
                        channel = ch_u8;
                        match message {
                            MidiMessage::ProgramChange { program: prog } => {
                                program = prog.as_int();
                                all_playback_events.push(PlaybackEvent {
                                    timestamp_secs: current_time,
                                    tick: track_tick,
                                    track_id: track_idx,
                                    channel: ch_u8,
                                    kind: PlaybackEventKind::ProgramChange { program },
                                });
                            }
                            MidiMessage::NoteOn { key, vel } => {
                                let pitch = key.as_int();
                                let velocity = vel.as_int();
                                if velocity > 0 {
                                    // Start of note
                                    if let Some((old_tick, old_time, old_vel)) = active_notes.insert((ch_u8, pitch), (track_tick, current_time, velocity)) {
                                        // Previous note wasn't turned off before new one started
                                        track_notes.push(Note {
                                            pitch,
                                            velocity: old_vel,
                                            start_secs: old_time,
                                            end_secs: current_time,
                                            start_tick: old_tick,
                                            end_tick: track_tick,
                                            track_id: track_idx,
                                            channel: ch_u8,
                                        });
                                    }
                                    all_playback_events.push(PlaybackEvent {
                                        timestamp_secs: current_time,
                                        tick: track_tick,
                                        track_id: track_idx,
                                        channel: ch_u8,
                                        kind: PlaybackEventKind::NoteOn { pitch, velocity },
                                    });
                                } else {
                                    // NoteOn with velocity 0 is NoteOff
                                    if let Some((start_tick, start_secs, note_vel)) = active_notes.remove(&(ch_u8, pitch)) {
                                        track_notes.push(Note {
                                            pitch,
                                            velocity: note_vel,
                                            start_secs,
                                            end_secs: current_time,
                                            start_tick,
                                            end_tick: track_tick,
                                            track_id: track_idx,
                                            channel: ch_u8,
                                        });
                                    }
                                    all_playback_events.push(PlaybackEvent {
                                        timestamp_secs: current_time,
                                        tick: track_tick,
                                        track_id: track_idx,
                                        channel: ch_u8,
                                        kind: PlaybackEventKind::NoteOff { pitch },
                                    });
                                }
                            }
                            MidiMessage::NoteOff { key, .. } => {
                                let pitch = key.as_int();
                                if let Some((start_tick, start_secs, velocity)) = active_notes.remove(&(ch_u8, pitch)) {
                                    track_notes.push(Note {
                                        pitch,
                                        velocity,
                                        start_secs,
                                        end_secs: current_time,
                                        start_tick,
                                        end_tick: track_tick,
                                        track_id: track_idx,
                                        channel: ch_u8,
                                    });
                                }
                                all_playback_events.push(PlaybackEvent {
                                    timestamp_secs: current_time,
                                    tick: track_tick,
                                    track_id: track_idx,
                                    channel: ch_u8,
                                    kind: PlaybackEventKind::NoteOff { pitch },
                                });
                            }
                            MidiMessage::Controller { controller, value } => {
                                all_playback_events.push(PlaybackEvent {
                                    timestamp_secs: current_time,
                                    tick: track_tick,
                                    track_id: track_idx,
                                    channel: ch_u8,
                                    kind: PlaybackEventKind::ControlChange {
                                        controller: controller.as_int(),
                                        value: value.as_int(),
                                    },
                                });
                            }
                            MidiMessage::PitchBend { bend } => {
                                all_playback_events.push(PlaybackEvent {
                                    timestamp_secs: current_time,
                                    tick: track_tick,
                                    track_id: track_idx,
                                    channel: ch_u8,
                                    kind: PlaybackEventKind::PitchBend {
                                        value: bend.as_int(),
                                    },
                                });
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }

            // Close any hanging notes
            let end_time = tick_to_secs(track_tick, ticks_per_beat, &tempo_markers);
            for ((ch_u8, pitch), (start_tick, start_secs, vel)) in active_notes {
                track_notes.push(Note {
                    pitch,
                    velocity: vel,
                    start_secs,
                    end_secs: end_time.max(start_secs + 0.1),
                    start_tick,
                    end_tick: track_tick,
                    track_id: track_idx,
                    channel: ch_u8,
                });
            }

            if track_tick > total_ticks {
                total_ticks = track_tick;
            }
            if end_time > total_duration_secs {
                total_duration_secs = end_time;
            }

            // Only retain tracks with notes or events
            let color = TRACK_PALETTE[track_idx % TRACK_PALETTE.len()];
            let instrument_name = if channel == 9 {
                "Standard Drum Kit".to_string()
            } else {
                gm_instrument_name(program).to_string()
            };

            parsed_tracks.push(Track {
                id: track_idx,
                name: track_name,
                channel,
                program,
                instrument_name,
                notes: track_notes,
                color,
                muted: false,
                soloed: false,
                volume: 100,
            });
        }

        // Filter out completely empty metadata tracks if other playable tracks exist
        let non_empty_tracks: Vec<Track> = parsed_tracks
            .into_iter()
            .filter(|t| !t.notes.is_empty())
            .enumerate()
            .map(|(new_id, mut t)| {
                t.id = new_id;
                t.color = TRACK_PALETTE[new_id % TRACK_PALETTE.len()];
                for note in &mut t.notes {
                    note.track_id = new_id;
                }
                t
            })
            .collect();

        // Sort all playback events chronologically
        all_playback_events.sort_by(|a, b| {
            a.timestamp_secs
                .partial_cmp(&b.timestamp_secs)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let initial_bpm = tempo_markers.first().map(|tm| tm.bpm).unwrap_or(120.0);

        Ok(Song {
            title: song_title,
            tracks: non_empty_tracks,
            duration_secs: total_duration_secs,
            total_ticks,
            ticks_per_beat,
            initial_bpm,
            tempo_changes: tempo_markers,
            time_signatures,
            events: all_playback_events,
        })
    }

    /// Generate an expressive multi-track demo song (Electric Guitar, Bass, Drums, Melody)
    pub fn create_demo_song() -> Song {
        let bpm = 128.0;
        let ticks_per_beat: u16 = 480;
        let beat_sec = 60.0 / bpm;
        let total_measures = 16;
        let total_beats = total_measures * 4;
        let duration_secs = total_beats as f64 * beat_sec;

        let tempo_changes = vec![TempoMarker {
            tick: 0,
            timestamp_secs: 0.0,
            bpm,
            micros_per_beat: (60_000_000.0 / bpm) as u32,
        }];

        let time_signatures = vec![TimeSignatureMarker {
            tick: 0,
            timestamp_secs: 0.0,
            numerator: 4,
            denominator: 4,
        }];

        let mut events = Vec::new();
        let mut tracks = Vec::new();

        // Track 0: Electric Overdriven Guitar (Chords & Riffs)
        let mut guitar_notes = Vec::new();
        let chord_progressions: [&[u8]; 4] = [
            &[57, 60, 64], // Am (A3, C4, E4)
            &[53, 57, 60], // F  (F3, A3, C4)
            &[55, 59, 62], // G  (G3, B3, D4)
            &[52, 55, 59], // Em (E3, G3, B3)
        ];

        for m in 0..total_measures {
            let chord = chord_progressions[(m % 4) as usize];
            let measure_start = m as f64 * 4.0 * beat_sec;
            // 8th note rhythm
            for eighth in 0..8 {
                let note_start = measure_start + (eighth as f64 * 0.5 * beat_sec);
                let note_end = note_start + (0.42 * beat_sec);
                let pitch = chord[eighth % chord.len()];
                guitar_notes.push(Note {
                    pitch,
                    velocity: 95,
                    start_secs: note_start,
                    end_secs: note_end,
                    start_tick: (note_start / beat_sec * ticks_per_beat as f64) as u64,
                    end_tick: (note_end / beat_sec * ticks_per_beat as f64) as u64,
                    track_id: 0,
                    channel: 0,
                });
            }
        }

        // Track 1: Electric Bass (Driving Root Notes)
        let mut bass_notes = Vec::new();
        let bass_roots: [u8; 4] = [33, 29, 31, 28]; // A1, F1, G1, E1
        for m in 0..total_measures {
            let root = bass_roots[(m % 4) as usize];
            let measure_start = m as f64 * 4.0 * beat_sec;
            for beat in 0..4 {
                let note_start = measure_start + (beat as f64 * beat_sec);
                let note_end = note_start + (0.85 * beat_sec);
                bass_notes.push(Note {
                    pitch: root,
                    velocity: 105,
                    start_secs: note_start,
                    end_secs: note_end,
                    start_tick: (note_start / beat_sec * ticks_per_beat as f64) as u64,
                    end_tick: (note_end / beat_sec * ticks_per_beat as f64) as u64,
                    track_id: 1,
                    channel: 1,
                });
            }
        }

        // Track 2: Lead Synth / Voice Melody
        let mut lead_notes = Vec::new();
        let melody = [
            (69, 1.0), (72, 1.0), (71, 0.5), (69, 0.5), (67, 1.0), // A4, C5, B4, A4, G4
            (65, 1.5), (67, 0.5), (69, 2.0),                        // F4, G4, A4
            (71, 1.0), (74, 1.0), (72, 0.5), (71, 0.5), (69, 1.0), // B4, D5, C5, B4, A4
            (67, 1.5), (64, 0.5), (69, 2.0),                        // G4, E4, A4
        ];
        let mut cur_time = 0.0;
        while cur_time < duration_secs - 2.0 {
            for &(pitch, len_beats) in &melody {
                let note_start = cur_time;
                let note_end = note_start + (len_beats * beat_sec * 0.9);
                lead_notes.push(Note {
                    pitch,
                    velocity: 110,
                    start_secs: note_start,
                    end_secs: note_end,
                    start_tick: (note_start / beat_sec * ticks_per_beat as f64) as u64,
                    end_tick: (note_end / beat_sec * ticks_per_beat as f64) as u64,
                    track_id: 2,
                    channel: 2,
                });
                cur_time += len_beats * beat_sec;
                if cur_time >= duration_secs - 2.0 {
                    break;
                }
            }
        }

        // Track 3: Drums (Kick, Snare, Hi-Hat)
        let mut drum_notes = Vec::new();
        for m in 0..total_measures {
            let measure_start = m as f64 * 4.0 * beat_sec;
            for beat in 0..4 {
                let beat_start = measure_start + (beat as f64 * beat_sec);
                // Kick on 1 and 3 (pitches 35 or 36)
                if beat == 0 || beat == 2 {
                    drum_notes.push(Note {
                        pitch: 36,
                        velocity: 115,
                        start_secs: beat_start,
                        end_secs: beat_start + 0.15,
                        start_tick: (beat_start / beat_sec * ticks_per_beat as f64) as u64,
                        end_tick: ((beat_start + 0.15) / beat_sec * ticks_per_beat as f64) as u64,
                        track_id: 3,
                        channel: 9,
                    });
                }
                // Snare on 2 and 4 (pitch 38)
                if beat == 1 || beat == 3 {
                    drum_notes.push(Note {
                        pitch: 38,
                        velocity: 110,
                        start_secs: beat_start,
                        end_secs: beat_start + 0.15,
                        start_tick: (beat_start / beat_sec * ticks_per_beat as f64) as u64,
                        end_tick: ((beat_start + 0.15) / beat_sec * ticks_per_beat as f64) as u64,
                        track_id: 3,
                        channel: 9,
                    });
                }
                // Closed Hi-Hat every eighth note (pitch 42)
                for eighth in 0..2 {
                    let hh_start = beat_start + (eighth as f64 * 0.5 * beat_sec);
                    drum_notes.push(Note {
                        pitch: 42,
                        velocity: 90,
                        start_secs: hh_start,
                        end_secs: hh_start + 0.1,
                        start_tick: (hh_start / beat_sec * ticks_per_beat as f64) as u64,
                        end_tick: ((hh_start + 0.1) / beat_sec * ticks_per_beat as f64) as u64,
                        track_id: 3,
                        channel: 9,
                    });
                }
            }
        }

        // Assemble tracks
        tracks.push(Track {
            id: 0,
            name: "Overdrive Guitar".to_string(),
            channel: 0,
            program: 29, // Overdriven Guitar
            instrument_name: "Overdriven Guitar".to_string(),
            notes: guitar_notes,
            color: TRACK_PALETTE[0],
            muted: false,
            soloed: false,
            volume: 100,
        });

        tracks.push(Track {
            id: 1,
            name: "Electric Bass".to_string(),
            channel: 1,
            program: 33, // Electric Bass (finger)
            instrument_name: "Finger Bass".to_string(),
            notes: bass_notes,
            color: TRACK_PALETTE[1],
            muted: false,
            soloed: false,
            volume: 100,
        });

        tracks.push(Track {
            id: 2,
            name: "Lead Synth".to_string(),
            channel: 2,
            program: 80, // Square Lead
            instrument_name: "Square Lead".to_string(),
            notes: lead_notes,
            color: TRACK_PALETTE[4],
            muted: false,
            soloed: false,
            volume: 100,
        });

        tracks.push(Track {
            id: 3,
            name: "Drums & Percussion".to_string(),
            channel: 9, // MIDI Drum Channel
            program: 0,
            instrument_name: "Standard Drum Kit".to_string(),
            notes: drum_notes,
            color: TRACK_PALETTE[5],
            muted: false,
            soloed: false,
            volume: 100,
        });

        // Generate playback events from all track notes
        for track in &tracks {
            // Program change at t=0
            events.push(PlaybackEvent {
                timestamp_secs: 0.0,
                tick: 0,
                track_id: track.id,
                channel: track.channel,
                kind: PlaybackEventKind::ProgramChange { program: track.program },
            });
            for note in &track.notes {
                events.push(PlaybackEvent {
                    timestamp_secs: note.start_secs,
                    tick: note.start_tick,
                    track_id: track.id,
                    channel: track.channel,
                    kind: PlaybackEventKind::NoteOn {
                        pitch: note.pitch,
                        velocity: note.velocity,
                    },
                });
                events.push(PlaybackEvent {
                    timestamp_secs: note.end_secs,
                    tick: note.end_tick,
                    track_id: track.id,
                    channel: track.channel,
                    kind: PlaybackEventKind::NoteOff { pitch: note.pitch },
                });
            }
        }

        events.sort_by(|a, b| {
            a.timestamp_secs
                .partial_cmp(&b.timestamp_secs)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Song {
            title: "Pink Pixel Demo Jam".to_string(),
            tracks,
            duration_secs,
            total_ticks: (total_beats as f64 * ticks_per_beat as f64) as u64,
            ticks_per_beat,
            initial_bpm: bpm,
            tempo_changes,
            time_signatures,
            events,
        }
    }
}

/// Convert a tick value to seconds using the piecewise tempo markers
fn tick_to_secs(tick: u64, ticks_per_beat: u16, tempo_markers: &[TempoMarker]) -> f64 {
    if tempo_markers.is_empty() {
        return (tick as f64 / ticks_per_beat as f64) * 0.5;
    }

    // Find the latest tempo marker at or before this tick
    let mut active_marker = &tempo_markers[0];
    for marker in tempo_markers {
        if marker.tick <= tick {
            active_marker = marker;
        } else {
            break;
        }
    }

    let delta_ticks = tick - active_marker.tick;
    let secs_per_tick = (active_marker.micros_per_beat as f64 / 1_000_000.0) / ticks_per_beat as f64;
    active_marker.timestamp_secs + (delta_ticks as f64 * secs_per_tick)
}
