use midi_tui::midi::{gm_instrument_name, MidiParser, Song};

#[test]
fn test_pitch_to_name() {
    assert_eq!(Song::pitch_to_name(60), "C4");  // Middle C
    assert_eq!(Song::pitch_to_name(69), "A4");  // Concert pitch 440 Hz
    assert_eq!(Song::pitch_to_name(61), "C#4");
    assert_eq!(Song::pitch_to_name(72), "C5");
    assert_eq!(Song::pitch_to_name(21), "A0");
}

#[test]
fn test_is_black_key() {
    assert!(!Song::is_black_key(60)); // C
    assert!(Song::is_black_key(61));  // C#
    assert!(!Song::is_black_key(62)); // D
    assert!(Song::is_black_key(63));  // D#
    assert!(!Song::is_black_key(64)); // E
    assert!(!Song::is_black_key(65)); // F
    assert!(Song::is_black_key(66));  // F#
}

#[test]
fn test_gm_instrument_name() {
    assert_eq!(gm_instrument_name(0), "Acoustic Grand");
    assert_eq!(gm_instrument_name(29), "Overdriven Guitar");
    assert_eq!(gm_instrument_name(33), "Finger Bass");
    assert_eq!(gm_instrument_name(80), "Lead 1 (square)");
}

#[test]
fn test_demo_song_structure() {
    let song = MidiParser::create_demo_song();
    assert_eq!(song.title, "Pink Pixel Demo Jam");
    assert_eq!(song.tracks.len(), 4);
    assert!(song.duration_secs > 20.0);
    assert!(!song.events.is_empty());

    // Check individual tracks
    let guitar = &song.tracks[0];
    assert_eq!(guitar.name, "Overdrive Guitar");
    assert!(!guitar.notes.is_empty());

    let bass = &song.tracks[1];
    assert_eq!(bass.name, "Electric Bass");
    assert!(!bass.notes.is_empty());

    let drums = &song.tracks[3];
    assert_eq!(drums.channel, 9);
    assert!(!drums.notes.is_empty());
}

#[test]
fn test_time_to_measure_beat() {
    let song = MidiParser::create_demo_song();
    // At t = 0.0, measure 1, beat 1.0
    let (m1, b1) = song.time_to_measure_beat(0.0);
    assert_eq!(m1, 1);
    assert!((b1 - 1.0).abs() < 0.01);

    // At 128 BPM, 1 beat = 60/128 = 0.46875s
    // 4 beats = 1.875s
    let (m2, b2) = song.time_to_measure_beat(1.875);
    assert_eq!(m2, 2);
    assert!((b2 - 1.0).abs() < 0.01);
}
