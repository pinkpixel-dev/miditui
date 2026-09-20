use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use anyhow::{Context, Result};
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};

pub enum SynthEngine {
    SoundFont(Box<Synthesizer>),
    Fallback(FallbackSynth),
}

impl SynthEngine {
    pub fn try_load_soundfont<P: AsRef<Path>>(sf2_path: P, sample_rate: u32) -> Result<Self> {
        let mut file = File::open(sf2_path.as_ref())
            .with_context(|| format!("Could not open SoundFont file at {:?}", sf2_path.as_ref()))?;
        let sound_font = Arc::new(
            SoundFont::new(&mut file)
                .map_err(|e| anyhow::anyhow!("Failed to parse SoundFont: {:?}", e))?,
        );
        let settings = SynthesizerSettings::new(sample_rate as i32);
        let synth = Synthesizer::new(&sound_font, &settings)
            .map_err(|e| anyhow::anyhow!("Failed to initialize synthesizer: {:?}", e))?;
        Ok(Self::SoundFont(Box::new(synth)))
    }

    pub fn fallback(sample_rate: u32) -> Self {
        Self::Fallback(FallbackSynth::new(sample_rate))
    }

    pub fn process_midi_message(&mut self, channel: i32, command: i32, data1: i32, data2: i32) {
        match self {
            Self::SoundFont(synth) => {
                synth.process_midi_message(channel, command, data1, data2);
            }
            Self::Fallback(synth) => {
                synth.process_midi_message(channel, command, data1, data2);
            }
        }
    }

    pub fn note_off_all(&mut self) {
        match self {
            Self::SoundFont(synth) => {
                synth.note_off_all(false);
            }
            Self::Fallback(synth) => {
                synth.all_notes_off();
            }
        }
    }

    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        match self {
            Self::SoundFont(synth) => {
                synth.render(left, right);
            }
            Self::Fallback(synth) => {
                synth.render(left, right);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Pure-Rust Polyphonic Fallback Synthesizer
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum EnvStage {
    Attack,
    Decay,
    Sustain,
    Release,
    Dead,
}

#[derive(Clone)]
struct Voice {
    active: bool,
    channel: u8,
    pitch: u8,
    velocity: f32,
    freq: f32,
    phase: f32,
    phase_inc: f32,
    is_drum: bool,
    noise_state: u32,
    stage: EnvStage,
    env_level: f32,
    attack_rate: f32,
    decay_rate: f32,
    sustain_level: f32,
    release_rate: f32,
    program: u8,
}

impl Voice {
    fn new() -> Self {
        Self {
            active: false,
            channel: 0,
            pitch: 60,
            velocity: 0.0,
            freq: 440.0,
            phase: 0.0,
            phase_inc: 0.0,
            is_drum: false,
            noise_state: 0x12345678,
            stage: EnvStage::Dead,
            env_level: 0.0,
            attack_rate: 0.01,
            decay_rate: 0.002,
            sustain_level: 0.6,
            release_rate: 0.005,
            program: 0,
        }
    }

    fn trigger(&mut self, channel: u8, pitch: u8, velocity: u8, program: u8, sample_rate: f32) {
        self.active = true;
        self.channel = channel;
        self.pitch = pitch;
        self.velocity = (velocity as f32 / 127.0).powf(1.4);
        self.program = program;
        self.is_drum = channel == 9;
        self.stage = EnvStage::Attack;
        self.env_level = 0.0;
        self.phase = 0.0;

        let freq = 440.0 * 2.0_f32.powf((pitch as f32 - 69.0) / 12.0);
        self.freq = freq;
        self.phase_inc = (freq * std::f32::consts::TAU) / sample_rate;

        // Custom envelope timing per instrument class
        if self.is_drum {
            // Percussion: snappy attack, quick exponential decay
            self.attack_rate = 1.0 / (0.002 * sample_rate);
            self.decay_rate = 1.0 / (0.12 * sample_rate);
            self.sustain_level = 0.0;
            self.release_rate = 1.0 / (0.05 * sample_rate);
        } else if (32..40).contains(&program) {
            // Bass: moderate punch
            self.attack_rate = 1.0 / (0.008 * sample_rate);
            self.decay_rate = 1.0 / (0.4 * sample_rate);
            self.sustain_level = 0.7;
            self.release_rate = 1.0 / (0.15 * sample_rate);
        } else if (80..96).contains(&program) {
            // Synth lead/pad: lush
            self.attack_rate = 1.0 / (0.03 * sample_rate);
            self.decay_rate = 1.0 / (0.3 * sample_rate);
            self.sustain_level = 0.8;
            self.release_rate = 1.0 / (0.25 * sample_rate);
        } else {
            // Standard piano/guitar
            self.attack_rate = 1.0 / (0.005 * sample_rate);
            self.decay_rate = 1.0 / (0.5 * sample_rate);
            self.sustain_level = 0.5;
            self.release_rate = 1.0 / (0.15 * sample_rate);
        }
    }

    fn release(&mut self) {
        if self.stage != EnvStage::Dead {
            self.stage = EnvStage::Release;
        }
    }

    fn next_sample(&mut self) -> f32 {
        if !self.active || self.stage == EnvStage::Dead {
            return 0.0;
        }

        // Envelope generator
        match self.stage {
            EnvStage::Attack => {
                self.env_level += self.attack_rate;
                if self.env_level >= 1.0 {
                    self.env_level = 1.0;
                    self.stage = EnvStage::Decay;
                }
            }
            EnvStage::Decay => {
                self.env_level -= self.decay_rate;
                if self.env_level <= self.sustain_level {
                    self.env_level = self.sustain_level;
                    self.stage = EnvStage::Sustain;
                }
            }
            EnvStage::Sustain => {
                if self.sustain_level <= 0.001 {
                    self.stage = EnvStage::Dead;
                    self.active = false;
                }
            }
            EnvStage::Release => {
                self.env_level -= self.release_rate;
                if self.env_level <= 0.0 {
                    self.env_level = 0.0;
                    self.stage = EnvStage::Dead;
                    self.active = false;
                }
            }
            EnvStage::Dead => {
                self.active = false;
                return 0.0;
            }
        }

        // Waveform generator
        let raw = if self.is_drum {
            // Pseudo-random noise with tone
            self.noise_state = self.noise_state.wrapping_mul(1664525).wrapping_add(1013904223);
            let noise = ((self.noise_state >> 16) as f32 / 32768.0) - 1.0;
            if self.pitch == 35 || self.pitch == 36 {
                // Bass drum: pitch drops quickly
                self.phase_inc *= 0.9992;
                self.phase.sin() * 0.8 + noise * 0.2
            } else if self.pitch == 38 || self.pitch == 40 {
                // Snare: noise mixed with body
                noise * 0.7 + (self.phase.sin() * 0.3)
            } else {
                // Cymbals/Hi-hat
                noise * 0.8
            }
        } else if (32..40).contains(&self.program) {
            // Bass: warm triangle / saw mix
            let tri = (2.0 * (self.phase / std::f32::consts::TAU) - 1.0).abs() * 2.0 - 1.0;
            let saw = (self.phase / std::f32::consts::TAU) * 2.0 - 1.0;
            tri * 0.6 + saw * 0.4
        } else if (80..88).contains(&self.program) {
            // Lead: square wave
            if self.phase.sin() >= 0.0 { 0.7 } else { -0.7 }
        } else {
            // Harmonic mix for acoustic instruments
            let fund = self.phase.sin();
            let h2 = (self.phase * 2.0).sin() * 0.35;
            let h3 = (self.phase * 3.0).sin() * 0.15;
            (fund + h2 + h3) * 0.7
        };

        self.phase += self.phase_inc;
        if self.phase >= std::f32::consts::TAU {
            self.phase -= std::f32::consts::TAU;
        }

        raw * self.env_level * self.velocity
    }
}

pub struct FallbackSynth {
    voices: Vec<Voice>,
    programs: [u8; 16],
    sample_rate: f32,
}

impl FallbackSynth {
    pub fn new(sample_rate: u32) -> Self {
        let max_polyphony = 32;
        let voices = (0..max_polyphony).map(|_| Voice::new()).collect();
        Self {
            voices,
            programs: [0; 16],
            sample_rate: sample_rate as f32,
        }
    }

    pub fn process_midi_message(&mut self, channel: i32, command: i32, data1: i32, data2: i32) {
        let ch = (channel.clamp(0, 15)) as u8;
        let cmd = command as u8;
        let d1 = data1 as u8;
        let d2 = data2 as u8;

        match cmd {
            // Note Off
            0x80 => {
                for v in &mut self.voices {
                    if v.active && v.channel == ch && v.pitch == d1 {
                        v.release();
                    }
                }
            }
            // Note On
            0x90 => {
                if d2 == 0 {
                    for v in &mut self.voices {
                        if v.active && v.channel == ch && v.pitch == d1 {
                            v.release();
                        }
                    }
                } else {
                    // Find free voice or steal oldest
                    let free_idx = self
                        .voices
                        .iter()
                        .position(|v| !v.active)
                        .unwrap_or_else(|| {
                            // Steal release stage voice first
                            self.voices
                                .iter()
                                .position(|v| v.stage == EnvStage::Release)
                                .unwrap_or(0)
                        });
                    let prog = self.programs[ch as usize];
                    self.voices[free_idx].trigger(ch, d1, d2, prog, self.sample_rate);
                }
            }
            // Program Change
            0xC0 => {
                self.programs[ch as usize] = d1;
            }
            // Control Change (All notes off: 0x7B)
            0xB0 if d1 == 0x7B || d1 == 0x78 => {
                self.all_notes_off();
            }
            _ => {}
        }
    }

    pub fn all_notes_off(&mut self) {
        for v in &mut self.voices {
            v.active = false;
            v.stage = EnvStage::Dead;
        }
    }

    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l_out, r_out) in left.iter_mut().zip(right.iter_mut()) {
            let mut sample = 0.0;
            for v in &mut self.voices {
                if v.active {
                    sample += v.next_sample();
                }
            }
            // Soft clipping limiter
            let limited = (sample * 0.35).tanh();
            *l_out = limited;
            *r_out = limited;
        }
    }
}
