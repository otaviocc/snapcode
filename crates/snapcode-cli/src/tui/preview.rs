// SPDX-License-Identifier: MIT
//! Choosing how to draw the preview image in the terminal.

use ratatui_image::picker::{Picker, ProtocolType};

pub const FALLBACK_FONT_SIZE: (u16, u16) = (8, 16);

const OVERRIDE_VAR: &str = "SNAPCODE_IMAGE_PROTOCOL";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewMode {
    pub protocol: ProtocolType,
    pub degraded: bool,
}

pub fn build_picker() -> (Picker, PreviewMode) {
    let font_size = detect_font_size().unwrap_or(FALLBACK_FONT_SIZE);
    let mut picker = Picker::from_fontsize(font_size);

    if let Some(protocol) = protocol_from_env() {
        picker.set_protocol_type(protocol);
    }

    let protocol = picker.protocol_type();
    (
        picker,
        PreviewMode {
            protocol,
            degraded: protocol == ProtocolType::Halfblocks,
        },
    )
}

fn detect_font_size() -> Option<(u16, u16)> {
    let size = crossterm::terminal::window_size().ok()?;
    if size.columns == 0 || size.rows == 0 || size.width == 0 || size.height == 0 {
        return None;
    }
    let cell = (size.width / size.columns, size.height / size.rows);
    (cell.0 > 0 && cell.1 > 0).then_some(cell)
}

pub fn protocol_from_env() -> Option<ProtocolType> {
    if let Some(forced) = std::env::var(OVERRIDE_VAR)
        .ok()
        .and_then(|v| parse_protocol(&v))
    {
        return Some(forced);
    }

    let var = |name: &str| std::env::var(name).unwrap_or_default().to_ascii_lowercase();
    let term = var("TERM");
    let term_program = var("TERM_PROGRAM");

    if std::env::var_os("TMUX").is_some() || term.starts_with("screen") {
        return Some(ProtocolType::Halfblocks);
    }

    if std::env::var_os("KITTY_WINDOW_ID").is_some()
        || term.contains("kitty")
        || term.contains("ghostty")
        || term_program == "ghostty"
        || term_program == "wezterm"
    {
        return Some(ProtocolType::Kitty);
    }

    if term_program == "iterm.app"
        || var("LC_TERMINAL").contains("iterm")
        || term_program == "warpterminal"
    {
        return Some(ProtocolType::Iterm2);
    }

    None
}

fn parse_protocol(value: &str) -> Option<ProtocolType> {
    match value.trim().to_ascii_lowercase().as_str() {
        "kitty" => Some(ProtocolType::Kitty),
        "iterm2" | "iterm" => Some(ProtocolType::Iterm2),
        "sixel" => Some(ProtocolType::Sixel),
        "halfblocks" | "blocks" | "ascii" => Some(ProtocolType::Halfblocks),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_detection_covers_the_common_terminals() {
        let saved: Vec<(&str, Option<String>)> = [
            OVERRIDE_VAR,
            "TMUX",
            "TERM",
            "TERM_PROGRAM",
            "KITTY_WINDOW_ID",
            "LC_TERMINAL",
        ]
        .iter()
        .map(|k| (*k, std::env::var(k).ok()))
        .collect();

        let clear = || {
            for (key, _) in &saved {
                std::env::remove_var(key);
            }
        };

        clear();
        std::env::set_var("TMUX", "/tmp/tmux-1");
        std::env::set_var(OVERRIDE_VAR, "kitty");
        assert_eq!(protocol_from_env(), Some(ProtocolType::Kitty));
        assert_eq!(protocol_from_env(), Some(ProtocolType::Kitty));

        clear();
        std::env::set_var("TMUX", "/tmp/tmux-1");
        std::env::set_var("TERM", "xterm-kitty");
        assert_eq!(protocol_from_env(), Some(ProtocolType::Halfblocks));

        clear();
        std::env::set_var("TERM", "screen.xterm-256color");
        assert_eq!(protocol_from_env(), Some(ProtocolType::Halfblocks));

        for (key, value) in [
            ("TERM", "xterm-kitty"),
            ("TERM", "xterm-ghostty"),
            ("KITTY_WINDOW_ID", "1"),
            ("TERM_PROGRAM", "WezTerm"),
            ("TERM_PROGRAM", "ghostty"),
        ] {
            clear();
            std::env::set_var(key, value);
            assert_eq!(
                protocol_from_env(),
                Some(ProtocolType::Kitty),
                "{key}={value} should mean kitty"
            );
        }

        for (key, value) in [("TERM_PROGRAM", "iTerm.app"), ("LC_TERMINAL", "iTerm2")] {
            clear();
            std::env::set_var(key, value);
            assert_eq!(
                protocol_from_env(),
                Some(ProtocolType::Iterm2),
                "{key}={value} should mean iterm2"
            );
        }

        clear();
        std::env::set_var("TERM", "xterm-256color");
        assert_eq!(protocol_from_env(), None);

        for (key, value) in saved {
            match value {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
    }

    #[test]
    fn override_values_parse_case_insensitively() {
        assert_eq!(parse_protocol("Kitty"), Some(ProtocolType::Kitty));
        assert_eq!(parse_protocol(" SIXEL "), Some(ProtocolType::Sixel));
        assert_eq!(parse_protocol("iterm"), Some(ProtocolType::Iterm2));
        assert_eq!(parse_protocol("blocks"), Some(ProtocolType::Halfblocks));
        assert_eq!(parse_protocol("nonsense"), None);
    }

    #[test]
    fn building_a_picker_never_panics() {
        let (picker, mode) = build_picker();
        assert!(picker.font_size().0 > 0);
        assert!(picker.font_size().1 > 0);
        assert_eq!(mode.degraded, mode.protocol == ProtocolType::Halfblocks);
    }
}
