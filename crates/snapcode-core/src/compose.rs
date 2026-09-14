// SPDX-License-Identifier: MIT
//! Turns configuration plus shaped text into a [`Scene`].

use std::path::Path;
use std::sync::Arc;

use crate::color::Rgba;
use crate::config::{Background, ImageFit, RenderConfig, ShadowConfig, TrafficLights};
use crate::font::FontStack;
use crate::highlight::DiffKind;
use crate::layout::{shape_label, ShapedRow, ShapedText};
use crate::scene::{Corners, ImageData, ImagePlacement, Op, Paint, PlacedGlyph, Rect, Scene};
use crate::theme::{ChromeTheme, MACOS_TRAFFIC_LIGHTS};

const TRAFFIC_LIGHT_INSET: f32 = 20.0;
const TRAFFIC_LIGHT_RADIUS: f32 = 6.0;
const TRAFFIC_LIGHT_SPACING: f32 = 8.0;

const TRANSLUCENT_OPACITY: f32 = 0.85;

#[derive(Debug, thiserror::Error)]
pub enum ComposeError {
    #[error("failed to read background image {}: {source}", path.display())]
    ImageIo {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to decode background image {}: {source}", path.display())]
    ImageDecode {
        path: std::path::PathBuf,
        #[source]
        source: image::ImageError,
    },
    #[error("the resulting image would be {width}x{height}, which exceeds the {limit}px limit")]
    TooLarge { width: u32, height: u32, limit: u32 },
}

const MAX_DIMENSION: u32 = 30_000;

pub fn compose(
    text: &ShapedText,
    config: &RenderConfig,
    chrome: &ChromeTheme,
    syntax_background: Rgba,
    fonts: &mut FontStack,
) -> Result<Scene, ComposeError> {
    let margin = config.px(config.window.margin);
    let padding = config.px(config.window.padding);
    let radius = config.px(config.window.radius);
    let titlebar = config
        .window
        .titlebar_height
        .map(|h| config.px(h))
        .unwrap_or(0.0);

    let gutter = compose_gutter(text, config, chrome, fonts);
    let gutter_width = gutter.as_ref().map_or(0.0, |g| g.total_width);

    let code_width = text.max_width;
    let window_width = (padding * 2.0 + gutter_width + code_width).ceil().max(1.0);
    let window_height = (titlebar + padding * 2.0 + text.height()).ceil().max(1.0);

    let canvas_width = (window_width + margin * 2.0).ceil();
    let canvas_height = (window_height + margin * 2.0).ceil();
    if canvas_width > MAX_DIMENSION as f32 || canvas_height > MAX_DIMENSION as f32 {
        return Err(ComposeError::TooLarge {
            width: canvas_width as u32,
            height: canvas_height as u32,
            limit: MAX_DIMENSION,
        });
    }

    let mut scene = Scene::new(canvas_width as u32, canvas_height as u32);
    let canvas = scene.bounds();
    let window = Rect::new(margin, margin, window_width, window_height);
    let window_corners = Corners::uniform(radius).clamped(&window);

    push_background(&mut scene, canvas, config, chrome)?;
    push_shadow(&mut scene, &window, window_corners, config, chrome);

    let opacity = if config.window.opaque {
        1.0
    } else {
        TRANSLUCENT_OPACITY
    };
    let window_color = chrome.window_color(syntax_background);
    let window_fill = window_color.with_opacity(opacity);
    scene.push(Op::Fill {
        rect: window,
        corners: window_corners,
        paint: Paint::Solid(window_fill),
    });

    if titlebar > 0.0 {
        push_titlebar(
            &mut scene,
            &window,
            titlebar,
            radius,
            config,
            chrome,
            fonts,
            syntax_background,
            opacity,
        );
    }

    let code_origin_x = window.x + padding + gutter_width;
    let code_origin_y = window.y + titlebar + padding;

    push_line_decorations(&mut scene, text, &window, code_origin_y, padding, chrome);

    if let Some(gutter) = gutter {
        push_gutter(
            &mut scene,
            gutter,
            text,
            window.x + padding,
            code_origin_y,
            &window,
            titlebar,
            padding,
            chrome,
        );
    }

    push_code(&mut scene, text, code_origin_x, code_origin_y);

    if config.window.border {
        scene.push(Op::Stroke {
            rect: window,
            corners: window_corners,
            color: chrome.border,
            width: config.scale.max(1.0),
        });
    }

    Ok(scene)
}

fn push_background(
    scene: &mut Scene,
    canvas: Rect,
    config: &RenderConfig,
    chrome: &ChromeTheme,
) -> Result<(), ComposeError> {
    let background = match &config.background {
        Background::Theme => &chrome.background,
        other => other,
    };

    let paint = match background {
        Background::Theme => Paint::Solid(Rgba::TRANSPARENT),
        Background::Solid { color } => Paint::Solid(*color),
        Background::Linear { stops, angle } => Paint::Linear {
            stops: crate::scene::normalize_stops(stops),
            angle: *angle,
        },
        Background::Radial { stops } => Paint::Radial {
            stops: crate::scene::normalize_stops(stops),
        },
        Background::Image {
            path,
            fit,
            blur,
            darken,
        } => Paint::Image {
            image: load_image(path)?,
            placement: placement_of(*fit),
            blur: config.px(*blur),
            darken: darken.clamp(-1.0, 1.0),
        },
    };

    scene.push(Op::Fill {
        rect: canvas,
        corners: Corners::SQUARE,
        paint,
    });
    Ok(())
}

fn placement_of(fit: ImageFit) -> ImagePlacement {
    match fit {
        ImageFit::Fill => ImagePlacement::Fill,
        ImageFit::Fit => ImagePlacement::Fit,
        ImageFit::Tile => ImagePlacement::Tile,
        ImageFit::Center => ImagePlacement::Center,
        ImageFit::Stretch => ImagePlacement::Stretch,
    }
}

fn load_image(path: &Path) -> Result<ImageData, ComposeError> {
    let bytes = std::fs::read(path).map_err(|source| ComposeError::ImageIo {
        path: path.to_path_buf(),
        source,
    })?;
    let decoded = image::load_from_memory(&bytes)
        .map_err(|source| ComposeError::ImageDecode {
            path: path.to_path_buf(),
            source,
        })?
        .to_rgba8();
    Ok(ImageData {
        width: decoded.width(),
        height: decoded.height(),
        pixels: Arc::new(decoded.into_raw()),
    })
}

fn push_shadow(
    scene: &mut Scene,
    window: &Rect,
    corners: Corners,
    config: &RenderConfig,
    chrome: &ChromeTheme,
) {
    let Some(shadow) = &config.shadow else {
        return;
    };
    let rect = window
        .inflate(config.px(shadow.spread))
        .translate(config.px(shadow.offset_x), config.px(shadow.offset_y));
    let color = if shadow.color == ShadowConfig::default().color {
        Rgba::new(
            chrome.shadow.r,
            chrome.shadow.g,
            chrome.shadow.b,
            shadow.color.a,
        )
    } else {
        shadow.color
    };
    scene.push(Op::Shadow {
        rect,
        corners: corners.clamped(&rect),
        blur: config.px(shadow.blur),
        color,
    });
}

#[allow(clippy::too_many_arguments)]
fn push_titlebar(
    scene: &mut Scene,
    window: &Rect,
    titlebar: f32,
    radius: f32,
    config: &RenderConfig,
    chrome: &ChromeTheme,
    fonts: &mut FontStack,
    syntax_background: Rgba,
    opacity: f32,
) {
    let bar = Rect::new(window.x, window.y, window.width, titlebar);
    scene.push(Op::Fill {
        rect: bar,
        corners: Corners::top(radius).clamped(window),
        paint: Paint::Solid(
            chrome
                .titlebar_color(syntax_background)
                .with_opacity(opacity),
        ),
    });

    let colors = match config.window.traffic_lights {
        TrafficLights::None => None,
        TrafficLights::Macos => Some(MACOS_TRAFFIC_LIGHTS),
        TrafficLights::Theme => Some(chrome.traffic_lights),
    };
    if let Some(colors) = colors {
        let radius = config.px(TRAFFIC_LIGHT_RADIUS);
        let spacing = config.px(TRAFFIC_LIGHT_SPACING);
        let inset = config.px(TRAFFIC_LIGHT_INSET);
        let pitch = radius * 2.0 + spacing;
        let center_y = bar.y + titlebar / 2.0;
        for (index, color) in colors.iter().enumerate() {
            let cx = bar.x + inset + radius + pitch * index as f32;
            scene.push(Op::Ellipse {
                rect: Rect::new(cx - radius, center_y - radius, radius * 2.0, radius * 2.0),
                color: *color,
            });
        }
    }

    if let Some(title) = config.window.title.as_deref().filter(|t| !t.is_empty()) {
        let font_size = config.px(config.font.size) * 0.8;
        let row = shape_label(title, chrome.title_text, font_size, titlebar, fonts);
        let reserved = match config.window.traffic_lights {
            TrafficLights::None => config.px(TRAFFIC_LIGHT_INSET),
            _ => {
                config.px(TRAFFIC_LIGHT_INSET)
                    + config.px(TRAFFIC_LIGHT_RADIUS) * 6.0
                    + config.px(TRAFFIC_LIGHT_SPACING) * 2.0
                    + config.px(TRAFFIC_LIGHT_INSET)
            }
        };
        let available = Rect::new(
            bar.x + reserved,
            bar.y,
            (bar.width - reserved * 2.0).max(0.0),
            titlebar,
        );
        if !available.is_empty() && row.width <= available.width {
            let x = available.x + (available.width - row.width) / 2.0;
            push_row(scene, &row, x, bar.y);
        }
    }
}

struct Gutter {
    rows: Vec<ShapedRow>,
    total_width: f32,
    number_width: f32,
    separator: bool,
    color: Rgba,
}

fn compose_gutter(
    text: &ShapedText,
    config: &RenderConfig,
    chrome: &ChromeTheme,
    fonts: &mut FontStack,
) -> Option<Gutter> {
    if !config.gutter.enabled {
        return None;
    }
    let color = config.gutter.color.unwrap_or(chrome.gutter);
    let font_size = config.px(config.font.size);
    let row_height = text.row_height;

    let mut rows = Vec::with_capacity(text.lines.len());
    let mut number_width: f32 = 0.0;
    for line in &text.lines {
        let number = config.gutter.start.saturating_add(line.number - 1);
        let row = shape_label(&number.to_string(), color, font_size, row_height, fonts);
        number_width = number_width.max(row.width);
        rows.push(row);
    }

    Some(Gutter {
        total_width: number_width + config.px(config.gutter.padding),
        number_width,
        separator: config.gutter.separator,
        color,
        rows,
    })
}

#[allow(clippy::too_many_arguments)]
fn push_gutter(
    scene: &mut Scene,
    gutter: Gutter,
    text: &ShapedText,
    origin_x: f32,
    origin_y: f32,
    window: &Rect,
    titlebar: f32,
    padding: f32,
    chrome: &ChromeTheme,
) {
    let mut row_index = 0usize;
    for (line, label) in text.lines.iter().zip(gutter.rows.iter()) {
        let y = origin_y + row_index as f32 * text.row_height;
        let x = origin_x + (gutter.number_width - label.width);
        push_row(scene, label, x, y);
        row_index += line.rows.len().max(1);
    }

    if gutter.separator {
        let x = origin_x + gutter.number_width + (gutter.total_width - gutter.number_width) / 2.0;
        let top = window.y + titlebar + padding / 2.0;
        let bottom = window.bottom() - padding / 2.0;
        scene.push(Op::Fill {
            rect: Rect::new(x, top, 1.0_f32.max(text.row_height / 28.0), bottom - top),
            corners: Corners::SQUARE,
            paint: Paint::Solid(chrome.gutter_separator),
        });
    }
    let _ = gutter.color;
}

fn push_line_decorations(
    scene: &mut Scene,
    text: &ShapedText,
    window: &Rect,
    origin_y: f32,
    padding: f32,
    chrome: &ChromeTheme,
) {
    let any_emphasis = text.lines.iter().any(|l| !l.emphasized);
    let mut row_index = 0usize;

    for line in &text.lines {
        let rows = line.rows.len().max(1);
        let y = origin_y + row_index as f32 * text.row_height;
        let height = rows as f32 * text.row_height;
        let band = Rect::new(window.x + padding / 2.0, y, window.width - padding, height);

        if let Some(diff) = line.diff {
            let color = match diff {
                DiffKind::Added => chrome.diff_added,
                DiffKind::Removed => chrome.diff_removed,
            };
            scene.push(Op::Fill {
                rect: band,
                corners: Corners::SQUARE,
                paint: Paint::Solid(color),
            });
        } else if any_emphasis && line.emphasized {
            scene.push(Op::Fill {
                rect: band,
                corners: Corners::SQUARE,
                paint: Paint::Solid(chrome.line_emphasis),
            });
        }

        row_index += rows;
    }
}

fn push_code(scene: &mut Scene, text: &ShapedText, origin_x: f32, origin_y: f32) {
    let mut glyphs = Vec::new();
    let mut row_index = 0usize;
    for line in &text.lines {
        for row in &line.rows {
            let y = origin_y + row_index as f32 * text.row_height + row.baseline;
            for glyph in &row.glyphs {
                glyphs.push(PlacedGlyph {
                    shaped: glyph.clone(),
                    origin_x,
                    origin_y: y,
                });
            }
            row_index += 1;
        }
        if line.rows.is_empty() {
            row_index += 1;
        }
    }
    scene.push(Op::Glyphs(glyphs));
}

fn push_row(scene: &mut Scene, row: &ShapedRow, x: f32, y: f32) {
    if row.glyphs.is_empty() {
        return;
    }
    let baseline = y + row.baseline;
    let glyphs = row
        .glyphs
        .iter()
        .map(|g| PlacedGlyph {
            shaped: g.clone(),
            origin_x: x,
            origin_y: baseline,
        })
        .collect();
    scene.push(Op::Glyphs(glyphs));
}
