// SPDX-License-Identifier: MIT
//! The settings model behind the TUI form.

use snapcode_core::config::{Background, GradientStop, RenderConfig, ShadowConfig, TrafficLights};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Theme,
    SyntaxTheme,
    Background,
    Language,
    LineNumbers,
    GutterSeparator,
    Diff,
    TabWidth,
    FontSize,
    LineHeight,
    Ligatures,
    Scale,
    Padding,
    Margin,
    Radius,
    Titlebar,
    TrafficLights,
    Border,
    Shadow,
    ShadowBlur,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Theme,
    Code,
    Type,
    Window,
}

impl Section {
    pub fn label(self) -> &'static str {
        match self {
            Section::Theme => "THEME",
            Section::Code => "CODE",
            Section::Type => "TYPE",
            Section::Window => "WINDOW",
        }
    }
}

pub const FIELDS: &[Field] = &[
    Field::Theme,
    Field::SyntaxTheme,
    Field::Background,
    Field::Language,
    Field::LineNumbers,
    Field::GutterSeparator,
    Field::Diff,
    Field::TabWidth,
    Field::FontSize,
    Field::LineHeight,
    Field::Ligatures,
    Field::Scale,
    Field::Padding,
    Field::Margin,
    Field::Radius,
    Field::Titlebar,
    Field::TrafficLights,
    Field::Border,
    Field::Shadow,
    Field::ShadowBlur,
];

#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    pub detected_language: &'a str,
    pub paired_syntax_theme: &'a str,
}

pub const BACKGROUND_PRESETS: &[&str] = &[
    "theme", "gradient", "sunset", "ocean", "slate", "black", "white", "none",
];

impl Field {
    pub fn section(self) -> Section {
        match self {
            Field::Theme | Field::SyntaxTheme | Field::Background => Section::Theme,
            Field::Language
            | Field::LineNumbers
            | Field::GutterSeparator
            | Field::Diff
            | Field::TabWidth => Section::Code,
            Field::FontSize | Field::LineHeight | Field::Ligatures => Section::Type,
            _ => Section::Window,
        }
    }

    pub fn is_picker(self) -> bool {
        matches!(self, Field::Theme | Field::SyntaxTheme | Field::Language)
    }

    pub fn label(self) -> &'static str {
        match self {
            Field::Theme => "Theme",
            Field::SyntaxTheme => "Syntax",
            Field::Background => "Background",
            Field::Language => "Language",
            Field::Scale => "Scale",
            Field::FontSize => "Font size",
            Field::LineHeight => "Line height",
            Field::Ligatures => "Ligatures",
            Field::Padding => "Padding",
            Field::Margin => "Margin",
            Field::Radius => "Radius",
            Field::Titlebar => "Titlebar",
            Field::TrafficLights => "Traffic lights",
            Field::Border => "Border",
            Field::Shadow => "Shadow",
            Field::ShadowBlur => "Shadow blur",
            Field::LineNumbers => "Line numbers",
            Field::GutterSeparator => "Gutter rule",
            Field::Diff => "Diff mode",
            Field::TabWidth => "Tab width",
        }
    }

    pub fn value(self, config: &RenderConfig, context: &Context<'_>) -> String {
        let on_off = |b: bool| if b { "on" } else { "off" }.to_string();
        match self {
            Field::Theme => config.theme.clone(),
            Field::SyntaxTheme => match config.syntax_theme.as_str() {
                "" => format!("{} (paired)", context.paired_syntax_theme),
                explicit => explicit.to_string(),
            },
            Field::Background => background_name(&config.background).to_string(),
            Field::Language => match &config.code.language {
                Some(explicit) => explicit.clone(),
                None => format!("{} (auto)", context.detected_language),
            },
            Field::Scale => format!("{:.1}x", config.scale),
            Field::FontSize => format!("{:.0}", config.font.size),
            Field::LineHeight => format!("{:.2}", config.font.line_height),
            Field::Ligatures => on_off(config.font.ligatures),
            Field::Padding => format!("{:.0}", config.window.padding),
            Field::Margin => format!("{:.0}", config.window.margin),
            Field::Radius => format!("{:.0}", config.window.radius),
            Field::Titlebar => match config.window.titlebar_height {
                Some(h) => format!("{h:.0}"),
                None => "off".into(),
            },
            Field::TrafficLights => match config.window.traffic_lights {
                TrafficLights::Macos => "macos".into(),
                TrafficLights::Theme => "theme".into(),
                TrafficLights::None => "none".into(),
            },
            Field::Border => on_off(config.window.border),
            Field::Shadow => on_off(config.shadow.is_some()),
            Field::ShadowBlur => match &config.shadow {
                Some(s) => format!("{:.0}", s.blur),
                None => "-".into(),
            },
            Field::LineNumbers => on_off(config.gutter.enabled),
            Field::GutterSeparator => on_off(config.gutter.separator),
            Field::Diff => on_off(config.code.diff),
            Field::TabWidth => config.code.tab_width.to_string(),
        }
    }

    pub fn adjust(
        self,
        config: &mut RenderConfig,
        delta: i32,
        chrome_themes: &[String],
        syntax_themes: &[String],
    ) {
        match self {
            Field::Theme => cycle_string(&mut config.theme, chrome_themes, delta),
            Field::SyntaxTheme => {
                cycle_optional_string(&mut config.syntax_theme, syntax_themes, delta)
            }
            Field::Language => {
                if delta < 0 {
                    config.code.language = None;
                }
            }
            Field::Background => {
                let current = background_name(&config.background);
                let next = cycle_slice(BACKGROUND_PRESETS, current, delta);
                config.background = background_from_name(next);
            }
            Field::Scale => {
                config.scale = step(config.scale, delta as f32 * 0.5, 0.5, 6.0);
                config.dpi = (72.0 * config.scale).round() as u32;
            }
            Field::FontSize => config.font.size = step(config.font.size, delta as f32, 6.0, 96.0),
            Field::LineHeight => {
                config.font.line_height =
                    step(config.font.line_height, delta as f32 * 0.05, 0.8, 3.0)
            }
            Field::Ligatures => config.font.ligatures = !config.font.ligatures,
            Field::Padding => {
                config.window.padding = step(config.window.padding, delta as f32 * 2.0, 0.0, 200.0)
            }
            Field::Margin => {
                config.window.margin = step(config.window.margin, delta as f32 * 4.0, 0.0, 400.0)
            }
            Field::Radius => {
                config.window.radius = step(config.window.radius, delta as f32, 0.0, 80.0)
            }
            Field::Titlebar => {
                config.window.titlebar_height = match config.window.titlebar_height {
                    Some(h) => {
                        let next = h + delta as f32 * 5.0;
                        (next >= 20.0).then_some(next.min(200.0))
                    }
                    None if delta > 0 => Some(50.0),
                    None => None,
                };
            }
            Field::TrafficLights => {
                const ORDER: [TrafficLights; 3] = [
                    TrafficLights::Theme,
                    TrafficLights::Macos,
                    TrafficLights::None,
                ];
                let index = ORDER
                    .iter()
                    .position(|t| *t == config.window.traffic_lights)
                    .unwrap_or(0);
                config.window.traffic_lights = ORDER[wrap(index, delta, ORDER.len())];
            }
            Field::Border => config.window.border = !config.window.border,
            Field::Shadow => {
                config.shadow = match config.shadow {
                    Some(_) => None,
                    None => Some(ShadowConfig::default()),
                }
            }
            Field::ShadowBlur => {
                if let Some(shadow) = &mut config.shadow {
                    shadow.blur = step(shadow.blur, delta as f32 * 4.0, 0.0, 200.0);
                }
            }
            Field::LineNumbers => config.gutter.enabled = !config.gutter.enabled,
            Field::GutterSeparator => {
                config.gutter.separator = !config.gutter.separator;
                if config.gutter.separator {
                    config.gutter.enabled = true;
                }
            }
            Field::Diff => config.code.diff = !config.code.diff,
            Field::TabWidth => {
                let next = config.code.tab_width as i32 + delta;
                config.code.tab_width = next.clamp(1, 16) as usize;
            }
        }
    }

    pub fn is_toggle(self) -> bool {
        matches!(
            self,
            Field::Ligatures
                | Field::Border
                | Field::Shadow
                | Field::LineNumbers
                | Field::GutterSeparator
                | Field::Diff
        )
    }
}

fn step(value: f32, delta: f32, min: f32, max: f32) -> f32 {
    let next = ((value + delta) * 1000.0).round() / 1000.0;
    next.clamp(min, max)
}

fn wrap(index: usize, delta: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let next = index as i32 + delta;
    next.rem_euclid(len as i32) as usize
}

fn cycle_string(current: &mut String, options: &[String], delta: i32) {
    if options.is_empty() {
        return;
    }
    let index = options.iter().position(|o| o == current).unwrap_or(0);
    *current = options[wrap(index, delta, options.len())].clone();
}

fn cycle_optional_string(current: &mut String, options: &[String], delta: i32) {
    let slots = options.len() + 1;
    let index = if current.is_empty() {
        0
    } else {
        options
            .iter()
            .position(|o| o == current)
            .map(|i| i + 1)
            .unwrap_or(0)
    };
    match wrap(index, delta, slots) {
        0 => current.clear(),
        next => current.clone_from(&options[next - 1]),
    }
}

fn cycle_slice<'a>(options: &[&'a str], current: &str, delta: i32) -> &'a str {
    let index = options.iter().position(|o| *o == current).unwrap_or(0);
    options[wrap(index, delta, options.len())]
}

pub fn background_name(background: &Background) -> &'static str {
    match background {
        Background::Theme => "theme",
        Background::Linear { .. } => "gradient",
        Background::Radial { .. } => "sunset",
        Background::Image { .. } => "image",
        Background::Solid { color } => match (color.r, color.g, color.b, color.a) {
            (_, _, _, 0) => "none",
            (0x0f, 0x17, 0x2a, _) => "slate",
            (0, 0, 0, _) => "black",
            (255, 255, 255, _) => "white",
            (0x0e, 0x74, 0x90, _) => "ocean",
            _ => "custom",
        },
    }
}

fn background_from_name(name: &str) -> Background {
    let color = |hex: &str| hex.parse().expect("preset colors are valid");
    let gradient = |a: &str, b: &str| {
        vec![
            GradientStop::new(0.0, color(a)),
            GradientStop::new(1.0, color(b)),
        ]
    };
    match name {
        "gradient" => Background::Linear {
            stops: gradient("#1e3a8a", "#701a75"),
            angle: 135.0,
        },
        "sunset" => Background::Radial {
            stops: gradient("#f97316", "#7c2d12"),
        },
        "ocean" => Background::Solid {
            color: color("#0e7490"),
        },
        "slate" => Background::Solid {
            color: color("#0f172a"),
        },
        "black" => Background::Solid {
            color: color("#000000"),
        },
        "white" => Background::Solid {
            color: color("#ffffff"),
        },
        "none" => Background::Solid {
            color: color("transparent"),
        },
        _ => Background::Theme,
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    fn themes() -> (Vec<String>, Vec<String>) {
        (
            vec!["warm".into(), "midnight".into(), "paper".into()],
            vec!["warm".into(), "Dracula".into()],
        )
    }

    fn adjust(config: &mut RenderConfig, field: Field, delta: i32) {
        let (chrome, syntax) = themes();
        field.adjust(config, delta, &chrome, &syntax);
    }

    const CTX: Context<'static> = Context {
        detected_language: "Swift",
        paired_syntax_theme: "warm",
    };

    #[test]
    fn every_field_has_a_label_and_a_value() {
        let config = RenderConfig::default();
        for field in FIELDS {
            assert!(!field.label().is_empty(), "{field:?} has no label");
            assert!(
                !field.value(&config, &CTX).is_empty(),
                "{field:?} has no value"
            );
        }
    }

    #[test]
    fn fields_are_grouped_into_contiguous_sections() {
        let mut seen: Vec<Section> = Vec::new();
        let mut current: Option<Section> = None;
        for field in FIELDS {
            let section = field.section();
            if current != Some(section) {
                assert!(
                    !seen.contains(&section),
                    "{section:?} is split across the list"
                );
                seen.push(section);
                current = Some(section);
            }
        }
        assert_eq!(seen.len(), 4, "expected four sections, got {seen:?}");
    }

    #[test]
    fn language_shows_what_detection_chose_until_it_is_overridden() {
        let mut config = RenderConfig::default();
        assert_eq!(Field::Language.value(&config, &CTX), "Swift (auto)");

        config.code.language = Some("Rust".into());
        assert_eq!(Field::Language.value(&config, &CTX), "Rust");
    }

    #[test]
    fn stepping_back_from_a_language_override_restores_detection() {
        let mut config = RenderConfig::default();
        config.code.language = Some("Rust".into());
        adjust(&mut config, Field::Language, -1);
        assert_eq!(config.code.language, None, "should return to automatic");
        adjust(&mut config, Field::Language, 1);
        assert_eq!(config.code.language, None);
    }

    #[test]
    fn picker_fields_are_the_ones_with_long_lists() {
        assert!(
            Field::Language.is_picker(),
            "220 languages cannot be cycled"
        );
        assert!(Field::SyntaxTheme.is_picker());
        assert!(Field::Theme.is_picker());
        assert!(!Field::Scale.is_picker());
        assert!(!Field::Border.is_picker());
        for field in FIELDS.iter().filter(|f| f.is_picker()) {
            assert!(!field.is_toggle(), "{field:?} cannot be both");
        }
    }

    #[test]
    fn field_list_has_no_duplicates() {
        let mut seen = Vec::new();
        for field in FIELDS {
            assert!(!seen.contains(field), "{field:?} listed twice");
            seen.push(*field);
        }
    }

    #[test]
    fn adjusting_any_field_keeps_the_config_valid() {
        for field in FIELDS {
            for delta in [-1, 1] {
                let mut config = RenderConfig::default();
                for _ in 0..60 {
                    adjust(&mut config, *field, delta);
                    config
                        .validate()
                        .unwrap_or_else(|e| panic!("{field:?} delta {delta} produced {e}"));
                }
            }
        }
    }

    #[test]
    fn the_syntax_theme_cycles_through_a_paired_slot() {
        let mut config = RenderConfig::default();
        assert_eq!(
            Field::SyntaxTheme.value(&config, &CTX),
            "warm (paired)",
            "an unset syntax theme should name the theme's pairing"
        );

        adjust(&mut config, Field::SyntaxTheme, 1);
        assert_eq!(config.syntax_theme, "warm");
        adjust(&mut config, Field::SyntaxTheme, 1);
        assert_eq!(config.syntax_theme, "Dracula");
        adjust(&mut config, Field::SyntaxTheme, 1);
        assert!(config.syntax_theme.is_empty(), "should wrap back to paired");
        adjust(&mut config, Field::SyntaxTheme, -1);
        assert_eq!(config.syntax_theme, "Dracula", "should wrap backwards");
    }

    #[test]
    fn theme_cycling_wraps_in_both_directions() {
        let mut config = RenderConfig::default();
        assert_eq!(config.theme, "warm");
        adjust(&mut config, Field::Theme, 1);
        assert_eq!(config.theme, "midnight");
        adjust(&mut config, Field::Theme, 1);
        assert_eq!(config.theme, "paper");
        adjust(&mut config, Field::Theme, 1);
        assert_eq!(config.theme, "warm", "should wrap around");
        adjust(&mut config, Field::Theme, -1);
        assert_eq!(config.theme, "paper", "should wrap backwards");
    }

    #[test]
    fn numeric_fields_clamp_rather_than_run_away() {
        let mut config = RenderConfig::default();
        for _ in 0..200 {
            adjust(&mut config, Field::Scale, 1);
        }
        assert_eq!(config.scale, 6.0);
        for _ in 0..200 {
            adjust(&mut config, Field::Scale, -1);
        }
        assert_eq!(config.scale, 0.5);

        for _ in 0..200 {
            adjust(&mut config, Field::Padding, -1);
        }
        assert_eq!(config.window.padding, 0.0, "padding must not go negative");
    }

    #[test]
    fn scale_keeps_dpi_in_step() {
        let mut config = RenderConfig::default();
        config.scale = 1.0;
        adjust(&mut config, Field::Scale, 1);
        assert_eq!(config.scale, 1.5);
        assert_eq!(config.dpi, 108);
    }

    #[test]
    fn fractional_steps_do_not_accumulate_float_dust() {
        let mut config = RenderConfig::default();
        config.font.line_height = 1.5;
        for _ in 0..10 {
            adjust(&mut config, Field::LineHeight, 1);
        }
        assert_eq!(config.font.line_height, 2.0);
        assert_eq!(Field::LineHeight.value(&config, &CTX), "2.00");
    }

    #[test]
    fn toggles_flip_regardless_of_direction() {
        for field in FIELDS.iter().filter(|f| f.is_toggle()) {
            let mut config = RenderConfig::default();
            let before = field.value(&config, &CTX);
            adjust(&mut config, *field, 1);
            let after = field.value(&config, &CTX);
            assert_ne!(before, after, "{field:?} did not toggle");
            adjust(&mut config, *field, -1);
            assert_eq!(
                field.value(&config, &CTX),
                before,
                "{field:?} did not flip back"
            );
        }
    }

    #[test]
    fn titlebar_turns_off_below_its_minimum() {
        let mut config = RenderConfig::default();
        for _ in 0..20 {
            adjust(&mut config, Field::Titlebar, -1);
        }
        assert_eq!(config.window.titlebar_height, None);
        assert_eq!(Field::Titlebar.value(&config, &CTX), "off");

        adjust(&mut config, Field::Titlebar, 1);
        assert_eq!(config.window.titlebar_height, Some(50.0));
    }

    #[test]
    fn shadow_blur_is_inert_without_a_shadow() {
        let mut config = RenderConfig::default();
        config.shadow = None;
        adjust(&mut config, Field::ShadowBlur, 1);
        assert!(config.shadow.is_none(), "must not resurrect the shadow");
        assert_eq!(Field::ShadowBlur.value(&config, &CTX), "-");
    }

    #[test]
    fn gutter_rule_implies_line_numbers() {
        let mut config = RenderConfig::default();
        assert!(!config.gutter.enabled);
        adjust(&mut config, Field::GutterSeparator, 1);
        assert!(config.gutter.separator);
        assert!(config.gutter.enabled, "a rule needs numbers beside it");
    }

    #[test]
    fn background_presets_round_trip_through_their_names() {
        for name in BACKGROUND_PRESETS {
            let background = background_from_name(name);
            assert_eq!(
                background_name(&background),
                *name,
                "preset {name:?} does not round-trip"
            );
        }
    }

    #[test]
    fn background_cycling_visits_every_preset() {
        let mut config = RenderConfig::default();
        let mut seen = Vec::new();
        for _ in 0..BACKGROUND_PRESETS.len() {
            seen.push(background_name(&config.background).to_string());
            adjust(&mut config, Field::Background, 1);
        }
        for preset in BACKGROUND_PRESETS {
            assert!(seen.contains(&preset.to_string()), "never saw {preset:?}");
        }
    }

    #[test]
    fn an_unknown_current_theme_does_not_panic() {
        let mut config = RenderConfig::default();
        config.theme = "deleted-theme".into();
        adjust(&mut config, Field::Theme, 1);
        assert!(themes().0.contains(&config.theme));
    }

    #[test]
    fn cycling_an_empty_theme_list_is_a_no_op() {
        let mut config = RenderConfig::default();
        Field::Theme.adjust(&mut config, 1, &[], &[]);
        assert_eq!(config.theme, "warm");
    }
}
