pub mod header;
pub mod help;
pub mod piano_roll;
pub mod theme;
pub mod track_list;
pub mod transport;

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Frame;

use header::render_header;
use help::render_help_modal;
use piano_roll::PianoRollWidget;
pub use theme::Theme;
use track_list::render_track_list;
use transport::TransportWidget;

use crate::app::App;

pub fn render(f: &mut Frame, app: &mut App, theme: &Theme) {
    let size = f.area();
    if size.width < 20 || size.height < 6 {
        return;
    }

    // Vertical split: Header (1), Main Area (min 0), Transport (1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(4),
            Constraint::Length(1),
        ])
        .split(size);

    let header_area = chunks[0];
    let main_area = chunks[1];
    let transport_area = chunks[2];

    // Horizontal split in Main Area: Track Sidebar vs Piano Roll
    let sidebar_width = if main_area.width > 90 {
        30
    } else if main_area.width > 60 {
        24
    } else {
        18
    };

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(sidebar_width),
            Constraint::Min(20),
        ])
        .split(main_area);

    let sidebar_area = main_chunks[0];
    let piano_roll_area = main_chunks[1];

    // Update viewport auto-follow when enabled
    app.update_auto_follow(piano_roll_area.width.saturating_sub(6));

    // 1. Render Header
    render_header(f, app, theme, header_area);

    // 2. Render Track List Sidebar
    render_track_list(f, app, theme, sidebar_area);

    // 3. Render Piano Roll
    f.render_widget(
        PianoRollWidget {
            app,
            theme,
        },
        piano_roll_area,
    );

    // 4. Render Transport Bar
    f.render_widget(
        TransportWidget {
            app,
            theme,
        },
        transport_area,
    );

    // 5. Help modal overlay if active
    if app.show_help {
        render_help_modal(f, theme, size);
    }
}
