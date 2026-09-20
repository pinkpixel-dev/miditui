pub mod app;
pub mod audio;
pub mod midi;
pub mod ui;

use std::io::stdout;
use std::panic;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseButton,
    MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use app::App;
use audio::AudioEngine;
use midi::MidiParser;
use ui::Theme;

#[derive(Parser, Debug)]
#[command(
    name = "miditui",
    author = "Pink Pixel <admin@pinkpixel.dev>",
    version = "0.1.0",
    about = "A sleek terminal MIDI player with piano roll visualizer and SoundFont synthesis"
)]
struct Cli {
    /// Path to MIDI file (.mid, .midi). If omitted, plays the built-in multi-track demo song.
    file: Option<PathBuf>,

    /// Path to a General MIDI SoundFont (.sf2) file
    #[arg(short, long)]
    sf2: Option<String>,

    /// Initial volume level (0 to 100)
    #[arg(short, long, default_value_t = 85)]
    volume: u8,

    /// Inspect and print MIDI file summary, then exit
    #[arg(long)]
    info: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // 1. Load or generate song
    let song = match &cli.file {
        Some(path) => {
            println!("Loading MIDI file: {}", path.display());
            MidiParser::parse_file(path)?
        }
        None => {
            println!("No MIDI file specified. Loading built-in multi-track demo...");
            MidiParser::create_demo_song()
        }
    };

    // If --info mode, print metadata and exit cleanly
    if cli.info {
        print_song_info(&song);
        return Ok(());
    }

    // 2. Initialize Audio Engine
    let audio = match AudioEngine::new(song.clone(), cli.sf2.as_deref()) {
        Ok(engine) => {
            engine.set_volume(cli.volume);
            Some(engine)
        }
        Err(err) => {
            eprintln!("Warning: Audio initialization failed ({:#}). Running in visualizer mode.", err);
            None
        }
    };

    // 3. Set up terminal with clean panic handling
    setup_panic_hook();
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 4. Initialize App state
    let mut app = App::new(song, audio);
    let theme = Theme::default();

    // Auto-start playback
    app.toggle_play();

    // 5. Main event loop (60 FPS tick)
    let tick_rate = Duration::from_millis(16);

    let res = run_app(&mut terminal, &mut app, &theme, tick_rate);

    // 6. Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Application error: {:#}", err);
    }

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    theme: &Theme,
    tick_rate: Duration,
) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|f| ui::render(f, app, theme))?;

        if event::poll(tick_rate)? {
            match event::read()? {
                Event::Key(key) => {
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && (key.code == KeyCode::Char('c') || key.code == KeyCode::Char('q'))
                    {
                        app.should_quit = true;
                        break;
                    }

                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            if app.show_help {
                                app.show_help = false;
                            } else {
                                app.should_quit = true;
                            }
                        }
                        KeyCode::Char('?') => {
                            app.show_help = !app.show_help;
                        }
                        KeyCode::Char(' ') => {
                            app.toggle_play();
                        }
                        KeyCode::Left | KeyCode::Char('h') => {
                            app.seek_relative(-5.0);
                        }
                        KeyCode::Right | KeyCode::Char('l') => {
                            app.seek_relative(5.0);
                        }
                        KeyCode::Char('[') => {
                            app.seek_measure(-1);
                        }
                        KeyCode::Char(']') => {
                            app.seek_measure(1);
                        }
                        KeyCode::Home => {
                            app.seek(0.0);
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.scroll_pitch(2);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.scroll_pitch(-2);
                        }
                        KeyCode::Char('+') | KeyCode::Char('=') => {
                            app.zoom_in();
                        }
                        KeyCode::Char('-') | KeyCode::Char('_') => {
                            app.zoom_out();
                        }
                        KeyCode::Char('f') => {
                            app.follow_playhead = !app.follow_playhead;
                        }
                        KeyCode::Char('r') => {
                            app.loop_playback = !app.loop_playback;
                        }
                        KeyCode::Tab => {
                            if key.modifiers.contains(KeyModifiers::SHIFT) {
                                app.select_prev_track();
                            } else {
                                app.select_next_track();
                            }
                        }
                        KeyCode::BackTab => {
                            app.select_prev_track();
                        }
                        KeyCode::Char('m') => {
                            app.toggle_mute_selected();
                        }
                        KeyCode::Char('s') => {
                            app.toggle_solo_selected();
                        }
                        KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                            let track_num = (c as u8 - b'1') as usize;
                            app.toggle_mute_track(track_num);
                        }
                        KeyCode::Char('<') | KeyCode::Char(',') => {
                            app.adjust_volume(-5);
                        }
                        KeyCode::Char('>') | KeyCode::Char('.') => {
                            app.adjust_volume(5);
                        }
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        app.handle_mouse_click(mouse.column, mouse.row);
                    }
                    MouseEventKind::ScrollUp => {
                        app.scroll_pitch(2);
                    }
                    MouseEventKind::ScrollDown => {
                        app.scroll_pitch(-2);
                    }
                    _ => {}
                },
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }
    Ok(())
}

fn print_song_info(song: &midi::Song) {
    println!("\n=== MIDI File Summary ===");
    println!("Title: {}", song.title);
    let m = (song.duration_secs / 60.0).floor() as u32;
    let s = song.duration_secs % 60.0;
    println!("Duration: {:02}:{:04.1} ({:.1}s)", m, s, song.duration_secs);
    println!("Initial Tempo: {:.1} BPM", song.initial_bpm);
    println!("Ticks per beat: {}", song.ticks_per_beat);
    println!("Tracks ({}):", song.tracks.len());

    for track in &song.tracks {
        println!(
            "  [{:>2}] Channel {:>2} | {:<24} | {:<22} | {} notes",
            track.id + 1,
            track.channel + 1,
            track.name,
            track.instrument_name,
            track.notes.len()
        );
    }
    println!("=========================\n");
}

fn setup_panic_hook() {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(panic_info);
    }));
}
