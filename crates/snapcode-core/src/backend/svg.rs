// SPDX-License-Identifier: MIT
//! Serializes a [`Scene`] to SVG.

use std::fmt::Write as _;

use swash::scale::{outline::Outline, ScaleContext};
use zeno::Verb;

use crate::color::Rgba;
use crate::font::FontStack;
use crate::scene::{Corners, Op, Paint, PlacedGlyph, Rect, Scene};

#[derive(Debug, thiserror::Error)]
pub enum SvgError {
    #[error("failed to write SVG: {0}")]
    Format(#[from] std::fmt::Error),
}

pub fn render(scene: &Scene, fonts: &mut FontStack) -> Result<String, SvgError> {
    let mut out = String::with_capacity(16 * 1024);
    let mut defs = String::new();
    let mut body = String::new();
    let mut next_id = 0usize;
    let mut scaler = ScaleContext::new();

    for op in &scene.ops {
        write_op(&mut body, &mut defs, &mut next_id, op, fonts, &mut scaler)?;
    }

    write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#,
        w = scene.width,
        h = scene.height
    )?;
    if !defs.is_empty() {
        write!(out, "<defs>{defs}</defs>")?;
    }
    out.push_str(&body);
    out.push_str("</svg>");
    Ok(out)
}

fn write_op(
    body: &mut String,
    defs: &mut String,
    next_id: &mut usize,
    op: &Op,
    fonts: &mut FontStack,
    scaler: &mut ScaleContext,
) -> Result<(), SvgError> {
    match op {
        Op::Fill {
            rect,
            corners,
            paint,
        } => {
            let fill = paint_reference(paint, rect, defs, next_id)?;
            write!(
                body,
                r#"<path d="{}" fill="{}" fill-opacity="{}"/>"#,
                rounded_rect_path(rect, &corners.clamped(rect)),
                fill.paint,
                fmt_f32(fill.opacity)
            )?;
        }
        Op::Shadow {
            rect,
            corners,
            blur,
            color,
        } => {
            let id = take_id(next_id, "shadow");
            let pad = (blur * 3.0).ceil();
            write!(
                defs,
                r#"<filter id="{id}" x="{x}" y="{y}" width="{w}" height="{h}" filterUnits="userSpaceOnUse"><feGaussianBlur stdDeviation="{s}"/></filter>"#,
                x = fmt_f32(rect.x - pad),
                y = fmt_f32(rect.y - pad),
                w = fmt_f32(rect.width + pad * 2.0),
                h = fmt_f32(rect.height + pad * 2.0),
                s = fmt_f32(blur / 2.0),
            )?;
            write!(
                body,
                r#"<path d="{}" fill="{}" fill-opacity="{}" filter="url(#{id})"/>"#,
                rounded_rect_path(rect, &corners.clamped(rect)),
                hex_rgb(*color),
                fmt_f32(color.a as f32 / 255.0),
            )?;
        }
        Op::Stroke {
            rect,
            corners,
            color,
            width,
        } => {
            let inset = rect.inflate(-width / 2.0);
            write!(
                body,
                r#"<path d="{}" fill="none" stroke="{}" stroke-opacity="{}" stroke-width="{}"/>"#,
                rounded_rect_path(&inset, &corners.clamped(&inset)),
                hex_rgb(*color),
                fmt_f32(color.a as f32 / 255.0),
                fmt_f32(*width),
            )?;
        }
        Op::Ellipse { rect, color } => {
            write!(
                body,
                r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}" fill="{}" fill-opacity="{}"/>"#,
                fmt_f32(rect.x + rect.width / 2.0),
                fmt_f32(rect.y + rect.height / 2.0),
                fmt_f32(rect.width / 2.0),
                fmt_f32(rect.height / 2.0),
                hex_rgb(*color),
                fmt_f32(color.a as f32 / 255.0),
            )?;
        }
        Op::Glyphs(glyphs) => write_glyphs(body, glyphs, fonts, scaler)?,
    }
    Ok(())
}

struct PaintRef {
    paint: String,
    opacity: f32,
}

fn paint_reference(
    paint: &Paint,
    rect: &Rect,
    defs: &mut String,
    next_id: &mut usize,
) -> Result<PaintRef, SvgError> {
    let reference = match paint {
        Paint::Solid(color) => PaintRef {
            paint: hex_rgb(*color),
            opacity: color.a as f32 / 255.0,
        },
        Paint::Linear { stops, angle } => {
            let id = take_id(next_id, "lg");
            let (sin, cos) = angle.to_radians().sin_cos();
            let cx = rect.x + rect.width / 2.0;
            let cy = rect.y + rect.height / 2.0;
            let extent = (rect.width * cos.abs() + rect.height * sin.abs()) / 2.0;
            write!(
                defs,
                r#"<linearGradient id="{id}" gradientUnits="userSpaceOnUse" x1="{}" y1="{}" x2="{}" y2="{}">"#,
                fmt_f32(cx - cos * extent),
                fmt_f32(cy - sin * extent),
                fmt_f32(cx + cos * extent),
                fmt_f32(cy + sin * extent),
            )?;
            write_stops(defs, stops)?;
            defs.push_str("</linearGradient>");
            PaintRef {
                paint: format!("url(#{id})"),
                opacity: 1.0,
            }
        }
        Paint::Radial { stops } => {
            let id = take_id(next_id, "rg");
            let cx = rect.x + rect.width / 2.0;
            let cy = rect.y + rect.height / 2.0;
            let radius = (rect.width.powi(2) + rect.height.powi(2)).sqrt() / 2.0;
            write!(
                defs,
                r#"<radialGradient id="{id}" gradientUnits="userSpaceOnUse" cx="{}" cy="{}" r="{}">"#,
                fmt_f32(cx),
                fmt_f32(cy),
                fmt_f32(radius.max(f32::EPSILON)),
            )?;
            write_stops(defs, stops)?;
            defs.push_str("</radialGradient>");
            PaintRef {
                paint: format!("url(#{id})"),
                opacity: 1.0,
            }
        }
        Paint::Image { .. } => PaintRef {
            paint: "none".to_string(),
            opacity: 0.0,
        },
    };
    Ok(reference)
}

fn write_stops(defs: &mut String, stops: &[crate::config::GradientStop]) -> Result<(), SvgError> {
    for stop in stops {
        write!(
            defs,
            r#"<stop offset="{}" stop-color="{}" stop-opacity="{}"/>"#,
            fmt_f32(stop.offset),
            hex_rgb(stop.color),
            fmt_f32(stop.color.a as f32 / 255.0),
        )?;
    }
    Ok(())
}

fn write_glyphs(
    body: &mut String,
    glyphs: &[PlacedGlyph],
    fonts: &mut FontStack,
    scaler: &mut ScaleContext,
) -> Result<(), SvgError> {
    let mut run_color: Option<Rgba> = None;
    let mut run_path = String::new();

    let flush =
        |body: &mut String, color: Option<Rgba>, path: &mut String| -> Result<(), SvgError> {
            if let (Some(color), false) = (color, path.is_empty()) {
                write!(
                    body,
                    r#"<path d="{path}" fill="{}" fill-opacity="{}"/>"#,
                    hex_rgb(color),
                    fmt_f32(color.a as f32 / 255.0),
                )?;
            }
            path.clear();
            Ok(())
        };

    for placed in glyphs {
        let glyph = &placed.shaped.inner;
        let color = placed.shaped.color;
        if run_color != Some(color) {
            flush(body, run_color, &mut run_path)?;
            run_color = Some(color);
        }

        let Some(font) = fonts
            .system_mut()
            .get_font(glyph.font_id, glyph.font_weight)
        else {
            continue;
        };
        let mut glyph_scaler = scaler
            .builder(font.as_swash())
            .size(glyph.font_size)
            .hint(false)
            .build();
        let Some(outline) = glyph_scaler.scale_outline(glyph.glyph_id) else {
            continue;
        };

        let ox = placed.origin_x + glyph.x + glyph.font_size * glyph.x_offset;
        let oy = placed.origin_y + glyph.y - glyph.font_size * glyph.y_offset;
        append_outline(&mut run_path, &outline, ox, oy);
    }
    flush(body, run_color, &mut run_path)?;
    Ok(())
}

fn append_outline(out: &mut String, outline: &Outline, ox: f32, oy: f32) {
    let points = outline.points();
    let verbs = outline.verbs();
    let mut index = 0usize;
    let px = |p: &zeno::Point| fmt_f32(ox + p.x);
    let py = |p: &zeno::Point| fmt_f32(oy - p.y);

    for verb in verbs {
        match verb {
            Verb::MoveTo => {
                let p = &points[index];
                let _ = write!(out, "M{} {}", px(p), py(p));
                index += 1;
            }
            Verb::LineTo => {
                let p = &points[index];
                let _ = write!(out, "L{} {}", px(p), py(p));
                index += 1;
            }
            Verb::QuadTo => {
                let (c, p) = (&points[index], &points[index + 1]);
                let _ = write!(out, "Q{} {} {} {}", px(c), py(c), px(p), py(p));
                index += 2;
            }
            Verb::CurveTo => {
                let (c1, c2, p) = (&points[index], &points[index + 1], &points[index + 2]);
                let _ = write!(
                    out,
                    "C{} {} {} {} {} {}",
                    px(c1),
                    py(c1),
                    px(c2),
                    py(c2),
                    px(p),
                    py(p)
                );
                index += 3;
            }
            Verb::Close => out.push('Z'),
        }
    }
}

pub fn image_backgrounds_dropped(scene: &Scene) -> bool {
    scene.ops.iter().any(|op| {
        matches!(
            op,
            Op::Fill {
                paint: Paint::Image { .. },
                ..
            }
        )
    })
}

fn rounded_rect_path(rect: &Rect, corners: &Corners) -> String {
    let (l, t, r, b) = (rect.x, rect.y, rect.right(), rect.bottom());
    if corners.is_square() {
        return format!(
            "M{} {}H{}V{}H{}Z",
            fmt_f32(l),
            fmt_f32(t),
            fmt_f32(r),
            fmt_f32(b),
            fmt_f32(l)
        );
    }
    let (tl, tr, br, bl) = (
        corners.top_left,
        corners.top_right,
        corners.bottom_right,
        corners.bottom_left,
    );
    let arc = |radius: f32, x: f32, y: f32| {
        if radius > 0.0 {
            format!(
                "A{r} {r} 0 0 1 {x} {y}",
                r = fmt_f32(radius),
                x = fmt_f32(x),
                y = fmt_f32(y)
            )
        } else {
            format!("L{} {}", fmt_f32(x), fmt_f32(y))
        }
    };
    format!(
        "M{} {}L{} {}{}L{} {}{}L{} {}{}L{} {}{}Z",
        fmt_f32(l + tl),
        fmt_f32(t),
        fmt_f32(r - tr),
        fmt_f32(t),
        arc(tr, r, t + tr),
        fmt_f32(r),
        fmt_f32(b - br),
        arc(br, r - br, b),
        fmt_f32(l + bl),
        fmt_f32(b),
        arc(bl, l, b - bl),
        fmt_f32(l),
        fmt_f32(t + tl),
        arc(tl, l + tl, t),
    )
}

fn take_id(next: &mut usize, prefix: &str) -> String {
    *next += 1;
    format!("{prefix}{next}")
}

fn hex_rgb(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

fn fmt_f32(v: f32) -> String {
    if !v.is_finite() {
        return "0".to_string();
    }
    let rounded = (v * 1000.0).round() / 1000.0;
    if rounded == rounded.trunc() && rounded.abs() < 1e9 {
        format!("{}", rounded as i64)
    } else {
        let s = format!("{rounded:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Scene;

    #[test]
    fn formats_floats_compactly() {
        assert_eq!(fmt_f32(10.0), "10");
        assert_eq!(fmt_f32(10.5), "10.5");
        assert_eq!(fmt_f32(10.123456), "10.123");
        assert_eq!(fmt_f32(-0.0005), "-0.001");
        assert_eq!(fmt_f32(-0.0004), "0");
        assert_eq!(fmt_f32(-12.5), "-12.5");
        assert_eq!(fmt_f32(f32::NAN), "0");
        assert_eq!(fmt_f32(f32::INFINITY), "0");
    }

    #[test]
    fn square_corners_use_a_simple_path() {
        let path = rounded_rect_path(&Rect::new(0.0, 0.0, 10.0, 20.0), &Corners::SQUARE);
        assert_eq!(path, "M0 0H10V20H0Z");
        assert!(!path.contains('A'), "square rect should have no arcs");
    }

    #[test]
    fn rounded_corners_emit_arcs() {
        let path = rounded_rect_path(&Rect::new(0.0, 0.0, 100.0, 100.0), &Corners::uniform(10.0));
        assert_eq!(path.matches('A').count(), 4);
        assert!(path.starts_with("M10 0"));
        assert!(path.ends_with('Z'));
    }

    #[test]
    fn top_only_corners_emit_two_arcs() {
        let path = rounded_rect_path(&Rect::new(0.0, 0.0, 100.0, 50.0), &Corners::top(10.0));
        assert_eq!(path.matches('A').count(), 2, "{path}");
    }

    #[test]
    fn empty_scene_is_still_a_valid_document() {
        let scene = Scene::new(40, 30);
        let mut fonts = crate::font::FontStack::embedded_only(&Default::default()).unwrap();
        let svg = render(&scene, &mut fonts).unwrap();
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.contains(r#"width="40" height="30""#));
        assert!(svg.contains(r#"viewBox="0 0 40 30""#));
        assert!(svg.ends_with("</svg>"));
    }
}
