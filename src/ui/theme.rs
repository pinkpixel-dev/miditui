use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    pub bg: Color,
    pub surface: Color,
    pub surface_hover: Color,
    pub border: Color,
    pub border_focused: Color,
    pub text: Color,
    pub text_muted: Color,
    pub text_subtle: Color,
    pub playhead: Color,
    pub active_key: Color,
    pub white_key_bg: Color,
    pub white_key_fg: Color,
    pub black_key_bg: Color,
    pub black_key_fg: Color,
    pub accent: Color,
    pub success: Color,
    pub warning: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Rgb(18, 18, 20),           // Dark charcoal base (#121214)
            surface: Color::Rgb(24, 24, 27),      // Elevated surface (#18181B)
            surface_hover: Color::Rgb(39, 39, 42),// Hover surface (#27272A)
            border: Color::Rgb(45, 45, 52),       // Subtle slate border
            border_focused: Color::Rgb(161, 161, 170), // Crisp light focus border
            text: Color::Rgb(244, 244, 245),      // Crisp white text (#F4F4F5)
            text_muted: Color::Rgb(161, 161, 170),// Muted gray text (#A1A1AA)
            text_subtle: Color::Rgb(113, 113, 122), // Dim subtle text (#71717A)
            playhead: Color::Rgb(239, 68, 68),    // Vivid crimson playhead (#EF4444)
            active_key: Color::Rgb(250, 204, 21), // Golden highlight when note is pressed
            white_key_bg: Color::Rgb(228, 228, 231), // Clean light piano key
            white_key_fg: Color::Rgb(24, 24, 27),
            black_key_bg: Color::Rgb(30, 30, 35), // Dark sleek piano key
            black_key_fg: Color::Rgb(212, 212, 216),
            accent: Color::Rgb(56, 189, 248),     // Clean sky blue accent (#38BDF8)
            success: Color::Rgb(34, 197, 94),     // Emerald green
            warning: Color::Rgb(245, 158, 11),    // Amber
        }
    }
}

impl Theme {
    pub fn base_style(&self) -> Style {
        Style::default().bg(self.bg).fg(self.text)
    }

    pub fn surface_style(&self) -> Style {
        Style::default().bg(self.surface).fg(self.text)
    }

    pub fn muted_style(&self) -> Style {
        Style::default().fg(self.text_muted)
    }

    pub fn header_title(&self) -> Style {
        Style::default()
            .fg(self.text)
            .add_modifier(Modifier::BOLD)
    }

    pub fn active_indicator(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }
}
