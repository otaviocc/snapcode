// SPDX-License-Identifier: MIT
//! End-to-end rendering tests.

#![allow(clippy::field_reassign_with_default)]

use snapcode_core::backend::raster::Raster;
use snapcode_core::color::Rgba;
use snapcode_core::config::{Background, LineRange, RenderConfig, TrafficLights};
use snapcode_core::{RenderRequest, Renderer};

const SWIFT: &str = "struct Point {\n    let x: Int\n    let y: Int\n}\n";

fn render(config: &RenderConfig, source: &str) -> Raster {
    Renderer::hermetic()
        .render_raster(&RenderRequest::new(source, config))
        .expect("render should succeed")
}

fn assert_pixel(raster: &Raster, x: u32, y: u32, expected: Rgba, what: &str) {
    let actual = raster.pixel(x, y);
    let close = [
        (actual.r, expected.r),
        (actual.g, expected.g),
        (actual.b, expected.b),
        (actual.a, expected.a),
    ]
    .iter()
    .all(|(a, b)| a.abs_diff(*b) <= 2);
    assert!(
        close,
        "{what}: expected {expected:?} at ({x}, {y}), found {actual:?}"
    );
}

#[test]
fn default_render_has_the_geometry_the_config_describes() {
    let config = RenderConfig::default();
    let raster = render(&config, SWIFT);

    let margin = 80.0;
    let titlebar = 100.0;

    let window_height = raster.height as f32 - margin * 2.0;
    let rows = 4.0;
    let row_height = 18.0 * 2.0 * (28.0 / 18.0);
    let expected_height = titlebar + 60.0 * 2.0 / 2.0 * 2.0 + rows * row_height;
    assert!(
        (window_height - expected_height).abs() <= 1.0,
        "window height {window_height} should be {expected_height}"
    );

    assert_pixel(
        &raster,
        2,
        2,
        Rgba::rgb(0xC1, 0x5F, 0x3C),
        "background corner",
    );
    assert_pixel(
        &raster,
        raster.width / 2,
        margin as u32 + 10,
        Rgba::rgb(0x2d, 0x2d, 0x2d),
        "titlebar",
    );
    assert_pixel(
        &raster,
        raster.width / 2,
        (margin + titlebar + 20.0) as u32,
        Rgba::rgb(0x1a, 0x1a, 0x1a),
        "window body",
    );
}

#[test]
fn traffic_lights_are_drawn_where_the_layout_says() {
    let config = RenderConfig::default();
    let raster = render(&config, SWIFT);

    let margin = 80.0;
    let center_y = (margin + 100.0 / 2.0) as u32;
    let colors = [
        Rgba::rgb(0xC1, 0x5F, 0x3C),
        Rgba::rgb(0xD4, 0x76, 0x3F),
        Rgba::rgb(0xE8, 0x91, 0x42),
    ];
    for (index, expected) in colors.iter().enumerate() {
        let x = (margin + 40.0 + 12.0 + 40.0 * index as f32) as u32;
        assert_pixel(
            &raster,
            x,
            center_y,
            *expected,
            &format!("traffic light {index}"),
        );
    }
}

#[test]
fn disabling_the_titlebar_shortens_the_window_by_its_height() {
    let with = render(&RenderConfig::default(), SWIFT);

    let mut config = RenderConfig::default();
    config.window.titlebar_height = None;
    let without = render(&config, SWIFT);

    assert_eq!(with.width, without.width, "width should not change");
    assert_eq!(
        with.height - without.height,
        100,
        "removing a 50-unit titlebar at scale 2 should remove 100 pixels"
    );
    assert_pixel(
        &without,
        (80.0 + 52.0) as u32,
        (80.0 + 12.0) as u32,
        Rgba::rgb(0x1a, 0x1a, 0x1a),
        "no traffic lights without a titlebar",
    );
}

#[test]
fn scale_multiplies_dimensions_proportionally() {
    let mut config = RenderConfig::default();
    config.scale = 1.0;
    let base = render(&config, SWIFT);

    config.scale = 2.0;
    let doubled = render(&config, SWIFT);
    config.scale = 3.0;
    let tripled = render(&config, SWIFT);

    for (raster, factor) in [(&doubled, 2.0_f32), (&tripled, 3.0)] {
        let expected_w = base.width as f32 * factor;
        let expected_h = base.height as f32 * factor;
        assert!(
            (raster.width as f32 - expected_w).abs() <= factor * 2.0,
            "width at {factor}x: {} vs ~{expected_w}",
            raster.width
        );
        assert!(
            (raster.height as f32 - expected_h).abs() <= factor * 2.0,
            "height at {factor}x: {} vs ~{expected_h}",
            raster.height
        );
    }
}

#[test]
fn the_window_grows_with_the_longest_line() {
    let short = render(&RenderConfig::default(), "x\n");
    let long = render(&RenderConfig::default(), "xxxxxxxxxxxxxxxxxxxx\n");
    assert!(long.width > short.width);
    assert_eq!(long.height, short.height, "one line either way");

    let tall = render(&RenderConfig::default(), "x\nx\nx\n");
    assert!(tall.height > short.height);
    assert_eq!(tall.width, short.width);
}

#[test]
fn indentation_is_preserved_in_the_output_width() {
    let plain = render(&RenderConfig::default(), "x\n");
    let indented = render(&RenderConfig::default(), "        x\n");
    assert!(
        indented.width > plain.width,
        "indentation was collapsed: {} vs {}",
        indented.width,
        plain.width
    );
}

#[test]
fn line_numbers_widen_the_window_without_changing_its_height() {
    let bare = render(&RenderConfig::default(), SWIFT);

    let mut config = RenderConfig::default();
    config.gutter.enabled = true;
    let numbered = render(&config, SWIFT);

    assert!(numbered.width > bare.width, "the gutter should take space");
    assert_eq!(numbered.height, bare.height, "the gutter is not vertical");
}

#[test]
fn a_transparent_background_leaves_the_margin_clear() {
    let mut config = RenderConfig::default();
    config.background = Background::Solid {
        color: Rgba::TRANSPARENT,
    };
    config.shadow = None;
    let raster = render(&config, SWIFT);

    assert_pixel(&raster, 1, 1, Rgba::TRANSPARENT, "transparent corner");
    assert_pixel(
        &raster,
        raster.width / 2,
        raster.height / 2,
        Rgba::rgb(0x1a, 0x1a, 0x1a),
        "window is still opaque",
    );
}

#[test]
fn the_shadow_darkens_the_background_near_the_window() {
    let mut config = RenderConfig::default();
    config.shadow = None;
    let without = render(&config, SWIFT);

    config = RenderConfig::default();
    let with = render(&config, SWIFT);
    assert_eq!((with.width, with.height), (without.width, without.height));

    let x = with.width / 2;
    let y = with.height - 20;
    assert!(
        with.pixel(x, y).luminance() < without.pixel(x, y).luminance(),
        "expected a shadow below the window: {:?} vs {:?}",
        with.pixel(x, y),
        without.pixel(x, y)
    );
}

#[test]
fn highlighting_dims_the_lines_outside_the_range() {
    let mut config = RenderConfig::default();
    config.code.highlight_lines = LineRange::parse_list("1").unwrap();
    let raster = render(&config, SWIFT);

    let x = 120;
    let margin_and_titlebar = 80 + 100;
    let padding = 60;
    let row_height = 56;
    let emphasized_y = margin_and_titlebar + padding + row_height / 2;
    let dimmed_y = emphasized_y + row_height * 2;

    assert!(
        raster.pixel(x, emphasized_y).luminance() > raster.pixel(x, dimmed_y).luminance(),
        "the highlighted row should be brighter than the rest"
    );
}

#[test]
fn diff_mode_tints_added_and_removed_lines_differently() {
    let mut config = RenderConfig::default();
    config.code.diff = true;
    let raster = render(&config, "+added\n-removed\n plain\n");

    let x = 120;
    let top = 80 + 100 + 60;
    let row_height = 56;
    let added = raster.pixel(x, top + row_height / 2);
    let removed = raster.pixel(x, top + row_height + row_height / 2);
    let plain = raster.pixel(x, top + row_height * 2 + row_height / 2);

    assert_ne!(added, plain, "an added line should be tinted");
    assert_ne!(removed, plain, "a removed line should be tinted");
    assert_ne!(added, removed, "the two tints should differ");
    assert!(added.g > added.r, "added should lean green: {added:?}");
    assert!(
        removed.r > removed.g,
        "removed should lean red: {removed:?}"
    );
}

#[test]
fn macos_traffic_lights_override_the_theme_colors() {
    let mut config = RenderConfig::default();
    config.window.traffic_lights = TrafficLights::Macos;
    let raster = render(&config, SWIFT);

    let center_y = (80.0 + 50.0) as u32;
    assert_pixel(
        &raster,
        (80.0 + 52.0) as u32,
        center_y,
        Rgba::rgb(0xff, 0x5f, 0x57),
        "macos red",
    );
    assert_pixel(
        &raster,
        (80.0 + 132.0) as u32,
        center_y,
        Rgba::rgb(0x28, 0xc8, 0x40),
        "macos green",
    );
}

#[test]
fn wrapped_lines_stay_inside_the_window() {
    let mut config = RenderConfig::default();
    config.scale = 1.0;
    config.code.max_width = Some(10);
    config.code.wrap = true;
    config.shadow = None;
    let raster = render(&config, "aaaa bbbb cccc dddd eeee\n");

    let margin = 40u32;
    let background = Rgba::rgb(0xC1, 0x5F, 0x3C);
    let window_bottom = raster.height - margin;

    for y in window_bottom..raster.height {
        for x in 0..raster.width {
            assert_eq!(
                raster.pixel(x, y),
                background,
                "text escaped the window at ({x}, {y}); a wrapped row was mispositioned"
            );
        }
    }

    let ink_rows = (margin..window_bottom)
        .filter(|y| (margin..raster.width - margin).any(|x| raster.pixel(x, *y) != background))
        .count();
    assert!(ink_rows > 0, "nothing was drawn inside the window");
}

#[test]
fn rendering_is_deterministic() {
    let config = RenderConfig::default();
    let first = render(&config, SWIFT);
    let second = render(&config, SWIFT);
    assert_eq!(first.pixels, second.pixels, "two renders must be identical");

    let renderer = Renderer::hermetic();
    let request = RenderRequest::new(SWIFT, &config);
    assert_eq!(
        renderer.render_png(&request).unwrap(),
        renderer.render_png(&request).unwrap()
    );
}

#[test]
fn awkward_inputs_still_render() {
    let config = RenderConfig::default();
    let cases: &[(&str, &str)] = &[
        ("empty", ""),
        ("only a newline", "\n"),
        ("no trailing newline", "let x = 1"),
        ("blank lines", "a\n\n\n\nb\n"),
        ("trailing whitespace", "let x = 1    \n"),
        ("tabs", "\tif x {\n\t\treturn\n\t}\n"),
        ("cjk", "let 名前 = \"日本語のテキスト\"\n"),
        ("emoji", "// 🎉 ship it 🚀\n"),
        ("rtl", "// مرحبا بالعالم\n"),
        ("ligature candidates", "if a != b && c => d {}\n"),
        ("very long line", &"x".repeat(400)),
        ("many lines", &"y\n".repeat(200)),
        ("crlf", "let a = 1\r\nlet b = 2\r\n"),
    ];

    for (name, source) in cases {
        let raster = Renderer::hermetic()
            .render_raster(&RenderRequest::new(source, &config))
            .unwrap_or_else(|e| panic!("{name:?} failed to render: {e}"));
        assert!(raster.width > 0 && raster.height > 0, "{name:?} was empty");
        assert_eq!(
            raster.pixels.len(),
            (raster.width * raster.height * 4) as usize,
            "{name:?} produced a malformed buffer"
        );
    }
}

#[test]
fn every_builtin_theme_renders() {
    let renderer = Renderer::hermetic();
    let chrome: Vec<String> = renderer
        .themes()
        .chrome_names()
        .into_iter()
        .map(str::to_string)
        .collect();

    for theme in chrome {
        let mut config = RenderConfig::default();
        config.theme = theme.clone();
        config.syntax_theme = renderer
            .themes()
            .chrome(&theme)
            .unwrap()
            .syntax_theme
            .clone();
        renderer
            .render_raster(&RenderRequest::new(SWIFT, &config))
            .unwrap_or_else(|e| panic!("theme {theme:?} failed: {e}"));
    }
}

#[test]
fn an_oversized_config_is_an_error_rather_than_an_allocation_bomb() {
    let mut config = RenderConfig::default();
    config.scale = 10.0;
    config.font.size = 400.0;
    let result =
        Renderer::hermetic().render_raster(&RenderRequest::new(&"x".repeat(10_000), &config));
    assert!(
        result.is_err(),
        "a 100k-pixel-wide render should be refused"
    );
}

#[test]
fn an_invalid_config_is_reported_before_any_work_happens() {
    let mut config = RenderConfig::default();
    config.scale = 0.0;
    assert!(Renderer::hermetic()
        .render_raster(&RenderRequest::new(SWIFT, &config))
        .is_err());
}

#[test]
fn an_unset_syntax_theme_follows_the_chrome_theme() {
    let mut paired = RenderConfig::default();
    paired.theme = "dracula".into();
    assert!(paired.syntax_theme.is_empty(), "the default must be unset");

    let mut explicit = paired.clone();
    explicit.syntax_theme = "Dracula".into();

    let mut other = paired.clone();
    other.syntax_theme = "warm".into();

    assert_eq!(
        render(&paired, SWIFT).pixels,
        render(&explicit, SWIFT).pixels,
        "an unset syntax theme should render as the theme's own pairing"
    );
    assert_ne!(
        render(&paired, SWIFT).pixels,
        render(&other, SWIFT).pixels,
        "an explicit syntax theme should still win"
    );
}
