use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};

use super::synth::SynthEngine;
use crate::midi::song::{PlaybackEventKind, Song};

pub enum AudioCommand {
    Play,
    Pause,
    TogglePlay,
    Seek(f64),
    SetVolume(u8),
    SetTrackMute(usize, bool),
    SetTrackSolo(usize, bool),
}

pub struct AudioEngine {
    _stream: Option<Stream>,
    pub is_playing: Arc<AtomicBool>,
    pub current_time_micros: Arc<AtomicU64>,
    pub volume_percent: Arc<AtomicU32>,
    pub command_tx: Sender<AudioCommand>,
    pub using_soundfont: bool,
    pub soundfont_name: Option<String>,
}

impl AudioEngine {
    pub fn new(song: Song, custom_sf2: Option<&str>) -> Result<Self> {
        let is_playing = Arc::new(AtomicBool::new(false));
        let current_time_micros = Arc::new(AtomicU64::new(0));
        let volume_percent = Arc::new(AtomicU32::new(85));
        let (cmd_tx, cmd_rx) = channel::<AudioCommand>();

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("No audio output device found on system")?;

        let supported_config = device
            .default_output_config()
            .context("Failed to get default output config")?;

        let sample_rate = supported_config.sample_rate();
        let channels = supported_config.channels() as usize;

        // Resolve SoundFont if available
        let sf2_path = find_soundfont(custom_sf2);
        let (synth, using_soundfont, soundfont_name) = match sf2_path {
            Some(path) => match SynthEngine::try_load_soundfont(&path, sample_rate) {
                Ok(engine) => {
                    let name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("SoundFont")
                        .to_string();
                    (engine, true, Some(name))
                }
                Err(_) => (SynthEngine::fallback(sample_rate), false, None),
            },
            None => (SynthEngine::fallback(sample_rate), false, None),
        };

        let is_playing_clone = Arc::clone(&is_playing);
        let current_time_clone = Arc::clone(&current_time_micros);
        let volume_clone = Arc::clone(&volume_percent);

        let stream = match supported_config.sample_format() {
            SampleFormat::F32 => Self::build_stream::<f32>(
                &device,
                supported_config.into(),
                channels,
                sample_rate,
                song,
                synth,
                is_playing_clone,
                current_time_clone,
                volume_clone,
                cmd_rx,
            )?,
            _ => anyhow::bail!("Unsupported audio output sample format"),
        };

        stream.play().context("Failed to start audio playback stream")?;

        Ok(Self {
            _stream: Some(stream),
            is_playing,
            current_time_micros,
            volume_percent,
            command_tx: cmd_tx,
            using_soundfont,
            soundfont_name,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn build_stream<T>(
        device: &cpal::Device,
        config: cpal::StreamConfig,
        channels: usize,
        sample_rate: u32,
        song: Song,
        mut synth: SynthEngine,
        is_playing: Arc<AtomicBool>,
        current_time_micros: Arc<AtomicU64>,
        volume_percent: Arc<AtomicU32>,
        cmd_rx: Receiver<AudioCommand>,
    ) -> Result<Stream>
    where
        T: cpal::Sample + cpal::FromSample<f32> + cpal::SizedSample,
    {
        let mut current_secs: f64 = 0.0;
        let mut event_cursor: usize = 0;
        let mut track_muted = vec![false; song.tracks.len()];
        let mut track_soloed = vec![false; song.tracks.len()];

        let err_fn = |err| eprintln!("Audio stream error: {}", err);

        let stream = device.build_output_stream(
            config,
            move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
                // Drain any pending commands from the UI
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        AudioCommand::Play => {
                            is_playing.store(true, Ordering::Relaxed);
                        }
                        AudioCommand::Pause => {
                            is_playing.store(false, Ordering::Relaxed);
                            synth.note_off_all();
                        }
                        AudioCommand::TogglePlay => {
                            let was_playing = is_playing.load(Ordering::Relaxed);
                            is_playing.store(!was_playing, Ordering::Relaxed);
                            if was_playing {
                                synth.note_off_all();
                            }
                        }
                        AudioCommand::Seek(target_secs) => {
                            current_secs = target_secs.clamp(0.0, song.duration_secs);
                            current_time_micros
                                .store((current_secs * 1_000_000.0) as u64, Ordering::Relaxed);
                            synth.note_off_all();
                            // Rewind and restore state
                            event_cursor = 0;
                            for (idx, ev) in song.events.iter().enumerate() {
                                if ev.timestamp_secs > current_secs {
                                    event_cursor = idx;
                                    break;
                                }
                                // Re-apply program and controller events up to seek point
                                match ev.kind {
                                    PlaybackEventKind::ProgramChange { program } => {
                                        synth.process_midi_message(
                                            ev.channel as i32,
                                            0xC0,
                                            program as i32,
                                            0,
                                        );
                                    }
                                    PlaybackEventKind::ControlChange { controller, value } => {
                                        synth.process_midi_message(
                                            ev.channel as i32,
                                            0xB0,
                                            controller as i32,
                                            value as i32,
                                        );
                                    }
                                    _ => {}
                                }
                            }
                        }
                        AudioCommand::SetVolume(vol) => {
                            volume_percent.store(vol as u32, Ordering::Relaxed);
                        }
                        AudioCommand::SetTrackMute(idx, muted) => {
                            if idx < track_muted.len() {
                                track_muted[idx] = muted;
                                if muted {
                                    synth.note_off_all();
                                }
                            }
                        }
                        AudioCommand::SetTrackSolo(idx, soloed) => {
                            if idx < track_soloed.len() {
                                track_soloed[idx] = soloed;
                                synth.note_off_all();
                            }
                        }
                    }
                }

                let frames = output.len() / channels;
                if !is_playing.load(Ordering::Relaxed) {
                    for sample in output.iter_mut() {
                        *sample = T::from_sample(0.0);
                    }
                    return;
                }

                let has_solo = track_soloed.iter().any(|&s| s);
                let dt_per_frame = 1.0 / sample_rate as f64;
                let buffer_dt = frames as f64 * dt_per_frame;
                let target_time = current_secs + buffer_dt;

                // Dispatch events that occur within this buffer window
                while event_cursor < song.events.len() {
                    let ev = &song.events[event_cursor];
                    if ev.timestamp_secs > target_time {
                        break;
                    }

                    // Check mute/solo logic
                    let track_id = ev.track_id;
                    let should_play = if has_solo {
                        track_soloed.get(track_id).copied().unwrap_or(false)
                    } else {
                        !track_muted.get(track_id).copied().unwrap_or(false)
                    };

                    match ev.kind {
                        PlaybackEventKind::NoteOn { pitch, velocity } => {
                            if should_play {
                                synth.process_midi_message(
                                    ev.channel as i32,
                                    0x90,
                                    pitch as i32,
                                    velocity as i32,
                                );
                            }
                        }
                        PlaybackEventKind::NoteOff { pitch } => {
                            synth.process_midi_message(
                                ev.channel as i32,
                                0x80,
                                pitch as i32,
                                0,
                            );
                        }
                        PlaybackEventKind::ProgramChange { program } => {
                            synth.process_midi_message(
                                ev.channel as i32,
                                0xC0,
                                program as i32,
                                0,
                            );
                        }
                        PlaybackEventKind::ControlChange { controller, value } => {
                            synth.process_midi_message(
                                ev.channel as i32,
                                0xB0,
                                controller as i32,
                                value as i32,
                            );
                        }
                        PlaybackEventKind::PitchBend { value } => {
                            let unsigned_val = (value as i32 + 8192).clamp(0, 16383);
                            let lsb = unsigned_val & 0x7F;
                            let msb = (unsigned_val >> 7) & 0x7F;
                            synth.process_midi_message(
                                ev.channel as i32,
                                0xE0,
                                lsb,
                                msb,
                            );
                        }
                        PlaybackEventKind::TempoChange { .. } => {}
                    }
                    event_cursor += 1;
                }

                // Render synth audio block
                let mut left = vec![0.0_f32; frames];
                let mut right = vec![0.0_f32; frames];
                synth.render(&mut left, &mut right);

                let vol = (volume_percent.load(Ordering::Relaxed) as f32 / 100.0).clamp(0.0, 1.0);

                if channels >= 2 {
                    for (i, frame) in output.chunks_mut(channels).enumerate() {
                        frame[0] = T::from_sample(left[i] * vol);
                        frame[1] = T::from_sample(right[i] * vol);
                        for extra in &mut frame[2..] {
                            *extra = T::from_sample(0.0);
                        }
                    }
                } else {
                    for (i, frame) in output.chunks_mut(1).enumerate() {
                        frame[0] = T::from_sample(((left[i] + right[i]) * 0.5) * vol);
                    }
                }

                current_secs += buffer_dt;
                if current_secs >= song.duration_secs {
                    current_secs = song.duration_secs;
                    is_playing.store(false, Ordering::Relaxed);
                    synth.note_off_all();
                }

                current_time_micros.store((current_secs * 1_000_000.0) as u64, Ordering::Relaxed);
            },
            err_fn,
            None,
        )?;

        Ok(stream)
    }

    pub fn get_current_time_secs(&self) -> f64 {
        self.current_time_micros.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Relaxed)
    }

    pub fn get_volume(&self) -> u8 {
        self.volume_percent.load(Ordering::Relaxed) as u8
    }

    pub fn toggle_play(&self) {
        let _ = self.command_tx.send(AudioCommand::TogglePlay);
    }

    pub fn seek(&self, target_secs: f64) {
        let _ = self.command_tx.send(AudioCommand::Seek(target_secs));
    }

    pub fn set_volume(&self, vol: u8) {
        let _ = self.command_tx.send(AudioCommand::SetVolume(vol));
    }

    pub fn set_track_mute(&self, track_idx: usize, muted: bool) {
        let _ = self.command_tx.send(AudioCommand::SetTrackMute(track_idx, muted));
    }

    pub fn set_track_solo(&self, track_idx: usize, soloed: bool) {
        let _ = self.command_tx.send(AudioCommand::SetTrackSolo(track_idx, soloed));
    }
}

/// Search common paths for SoundFont .sf2 files
pub fn find_soundfont(custom_path: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = custom_path {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    if let Ok(env_path) = std::env::var("SOUNDFONT") {
        let p = PathBuf::from(env_path);
        if p.exists() {
            return Some(p);
        }
    }

    let search_dirs = [
        PathBuf::from("/usr/share/sounds/sf2"),
        PathBuf::from("/usr/share/soundfonts"),
        dirs_home().map(|h| h.join(".local/share/soundfonts")).unwrap_or_default(),
        dirs_home().map(|h| h.join(".cache/miditui")).unwrap_or_default(),
        dirs_home().map(|h| h.join(".cache/midi-tui")).unwrap_or_default(),
    ];

    for dir in &search_dirs {
        if dir.exists() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("sf2") {
                        return Some(path);
                    }
                }
            }
        }
    }

    None
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}
