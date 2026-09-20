use midi_tui::app::App;
use midi_tui::midi::MidiParser;

#[test]
fn test_viewport_duration_zoom() {
    let song = MidiParser::create_demo_song();
    let mut app = App::new(song, None);

    // Default zoom = 1.0. At 40 columns: 40 / (4 * 1.0) = 10.0 seconds
    assert_eq!(app.viewport_duration(40), 10.0);

    // Zoom in: 2.0x -> duration should halve (5.0 seconds)
    app.zoom = 2.0;
    assert_eq!(app.viewport_duration(40), 5.0);

    // Zoom out: 0.5x -> duration should double (20.0 seconds)
    app.zoom = 0.5;
    assert_eq!(app.viewport_duration(40), 20.0);
}

#[test]
fn test_active_pitches_extraction() {
    let song = MidiParser::create_demo_song();
    let app = App::new(song, None);

    // At t = 0.1s, notes are actively sounding across tracks
    let active = app.get_active_pitches(0.1);
    assert!(!active.is_empty(), "Should have sounding notes at t=0.1s");
}

#[test]
fn test_mute_and_solo_state() {
    let song = MidiParser::create_demo_song();
    let mut app = App::new(song, None);

    // Initially nothing is muted or soloed
    assert!(!app.song.tracks[0].muted);
    assert!(!app.song.tracks[0].soloed);

    // Toggle mute on track 0
    app.toggle_mute_track(0);
    assert!(app.song.tracks[0].muted);

    // Toggle solo on track 1
    app.toggle_solo_track(1);
    assert!(app.song.tracks[1].soloed);

    // Active pitches should only come from soloed track 1
    let active = app.get_active_pitches(0.1);
    for pitch in active {
        let in_track_1 = app.song.tracks[1].notes.iter().any(|n| n.pitch == pitch);
        assert!(in_track_1, "When track 1 is soloed, active pitch must be from track 1");
    }
}
