use ratatui::style::Color;

#[derive(Debug, Clone)]
pub struct Note {
    pub pitch: u8,
    pub velocity: u8,
    pub start_secs: f64,
    pub end_secs: f64,
    pub start_tick: u64,
    pub end_tick: u64,
    pub track_id: usize,
    pub channel: u8,
}

#[derive(Debug, Clone)]
pub struct Track {
    pub id: usize,
    pub name: String,
    pub channel: u8,
    pub program: u8,
    pub instrument_name: String,
    pub notes: Vec<Note>,
    pub color: Color,
    pub muted: bool,
    pub soloed: bool,
    pub volume: u8,
}

#[derive(Debug, Clone)]
pub enum PlaybackEventKind {
    NoteOn { pitch: u8, velocity: u8 },
    NoteOff { pitch: u8 },
    ProgramChange { program: u8 },
    ControlChange { controller: u8, value: u8 },
    PitchBend { value: i16 },
    TempoChange { bpm: f64 },
}

#[derive(Debug, Clone)]
pub struct PlaybackEvent {
    pub timestamp_secs: f64,
    pub tick: u64,
    pub track_id: usize,
    pub channel: u8,
    pub kind: PlaybackEventKind,
}

#[derive(Debug, Clone)]
pub struct TempoMarker {
    pub tick: u64,
    pub timestamp_secs: f64,
    pub bpm: f64,
    pub micros_per_beat: u32,
}

#[derive(Debug, Clone)]
pub struct TimeSignatureMarker {
    pub tick: u64,
    pub timestamp_secs: f64,
    pub numerator: u8,
    pub denominator: u8,
}

#[derive(Debug, Clone)]
pub struct Song {
    pub title: String,
    pub tracks: Vec<Track>,
    pub duration_secs: f64,
    pub total_ticks: u64,
    pub ticks_per_beat: u16,
    pub initial_bpm: f64,
    pub tempo_changes: Vec<TempoMarker>,
    pub time_signatures: Vec<TimeSignatureMarker>,
    pub events: Vec<PlaybackEvent>,
}

impl Song {
    pub fn empty() -> Self {
        Self {
            title: "Untitled".to_string(),
            tracks: Vec::new(),
            duration_secs: 0.0,
            total_ticks: 0,
            ticks_per_beat: 480,
            initial_bpm: 120.0,
            tempo_changes: vec![TempoMarker {
                tick: 0,
                timestamp_secs: 0.0,
                bpm: 120.0,
                micros_per_beat: 500_000,
            }],
            time_signatures: vec![TimeSignatureMarker {
                tick: 0,
                timestamp_secs: 0.0,
                numerator: 4,
                denominator: 4,
            }],
            events: Vec::new(),
        }
    }

    /// Convert a timestamp in seconds to the current musical measure and beat (1-indexed).
    pub fn time_to_measure_beat(&self, time_secs: f64) -> (u32, f64) {
        if self.tempo_changes.is_empty() {
            return (1, 1.0);
        }

        // Find applicable tempo
        let mut accumulated_beats = 0.0;
        let mut last_t = 0.0;
        let mut current_bpm = self.initial_bpm;

        for tempo in &self.tempo_changes {
            if time_secs < tempo.timestamp_secs {
                break;
            }
            let dt = tempo.timestamp_secs - last_t;
            accumulated_beats += dt * (current_bpm / 60.0);
            last_t = tempo.timestamp_secs;
            current_bpm = tempo.bpm;
        }

        if time_secs > last_t {
            accumulated_beats += (time_secs - last_t) * (current_bpm / 60.0);
        }

        let num = self.time_signatures.first().map(|ts| ts.numerator).unwrap_or(4) as f64;
        let measure = (accumulated_beats / num).floor() as u32 + 1;
        let beat = (accumulated_beats % num) + 1.0;
        (measure, beat)
    }

    /// Pitch to standard scientific pitch notation (e.g. 60 -> "C4")
    pub fn pitch_to_name(pitch: u8) -> String {
        const NOTE_NAMES: [&str; 12] = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        let note = NOTE_NAMES[(pitch % 12) as usize];
        let octave = (pitch as i32 / 12) - 1;
        format!("{}{}", note, octave)
    }

    /// Whether a pitch is a black key on piano
    pub fn is_black_key(pitch: u8) -> bool {
        matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
    }
}

/// Standard General MIDI Instrument Names
pub fn gm_instrument_name(program: u8) -> &'static str {
    const GM_NAMES: [&str; 128] = [
        // Piano (0-7)
        "Acoustic Grand", "Bright Acoustic", "Electric Grand", "Honky-tonk",
        "Electric Piano 1", "Electric Piano 2", "Harpsichord", "Clavinet",
        // Chromatic Percussion (8-15)
        "Celesta", "Glockenspiel", "Music Box", "Vibraphone",
        "Marimba", "Xylophone", "Tubular Bells", "Dulcimer",
        // Organ (16-23)
        "Drawbar Organ", "Percussive Organ", "Rock Organ", "Church Organ",
        "Reed Organ", "Accordion", "Harmonica", "Tango Accordion",
        // Guitar (24-31)
        "Nylon Guitar", "Steel Guitar", "Jazz Guitar", "Clean Guitar",
        "Muted Guitar", "Overdriven Guitar", "Distortion Guitar", "Guitar Harmonics",
        // Bass (32-39)
        "Acoustic Bass", "Finger Bass", "Pick Bass", "Fretless Bass",
        "Slap Bass 1", "Slap Bass 2", "Synth Bass 1", "Synth Bass 2",
        // Strings (40-47)
        "Violin", "Viola", "Cello", "Contrabass",
        "Tremolo Strings", "Pizzicato Strings", "Orchestral Harp", "Timpani",
        // Ensemble (48-55)
        "String Ensemble 1", "String Ensemble 2", "Synth Strings 1", "Synth Strings 2",
        "Choir Aahs", "Voice Oohs", "Synth Choir", "Orchestra Hit",
        // Brass (56-63)
        "Trumpet", "Trombone", "Tuba", "Muted Trumpet",
        "French Horn", "Brass Section", "Synth Brass 1", "Synth Brass 2",
        // Reed (64-71)
        "Soprano Sax", "Alto Sax", "Tenor Sax", "Baritone Sax",
        "Oboe", "English Horn", "Bassoon", "Clarinet",
        // Pipe (72-79)
        "Piccolo", "Flute", "Recorder", "Pan Flute",
        "Blown Bottle", "Shakuhachi", "Whistle", "Ocarina",
        // Synth Lead (80-87)
        "Lead 1 (square)", "Lead 2 (sawtooth)", "Lead 3 (calliope)", "Lead 4 (chiff)",
        "Lead 5 (charang)", "Lead 6 (voice)", "Lead 7 (fifths)", "Lead 8 (bass+lead)",
        // Synth Pad (88-95)
        "Pad 1 (new age)", "Pad 2 (warm)", "Pad 3 (polysynth)", "Pad 4 (choir)",
        "Pad 5 (bowed)", "Pad 6 (metallic)", "Pad 7 (halo)", "Pad 8 (sweep)",
        // Synth Effects (96-103)
        "FX 1 (rain)", "FX 2 (soundtrack)", "FX 3 (crystal)", "FX 4 (atmosphere)",
        "FX 5 (brightness)", "FX 6 (goblins)", "FX 7 (echoes)", "FX 8 (sci-fi)",
        // Ethnic (104-111)
        "Sitar", "Banjo", "Shamisen", "Koto",
        "Kalimba", "Bag pipe", "Fiddle", "Shanai",
        // Percussive (112-119)
        "Tinkle Bell", "Agogo", "Steel Drums", "Woodblock",
        "Taiko Drum", "Melodic Tom", "Synth Drum", "Reverse Cymbal",
        // Sound Effects (120-127)
        "Guitar Fret Noise", "Breath Noise", "Seashore", "Bird Tweet",
        "Telephone Ring", "Helicopter", "Applause", "Gunshot",
    ];

    if (program as usize) < GM_NAMES.len() {
        GM_NAMES[program as usize]
    } else {
        "Unknown Instrument"
    }
}
