// SPDX-License-Identifier: MIT
//! Proves the SVG backend agrees with the raster backend.

#![allow(clippy::field_reassign_with_default)]

use snapcode_core::config::{Background, GradientStop, RenderConfig};
use snapcode_core::{RenderRequest, Renderer};

const SOURCE: &str = "struct Point {\n    let x: Int\n    // origin\n    let y: Int\n}\n";

fn rasterize_svg(svg: &str) -> (u32, u32, Vec<u8>) {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).expect("SVG must parse");
    let size = tree.size().to_int_size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height()).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );

    let mut out = Vec::with_capacity(pixmap.data().len());
    for px in pixmap.pixels() {
        let a = px.alpha();
        if a == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        let un = |c: u8| ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
        out.extend_from_slice(&[un(px.red()), un(px.green()), un(px.blue()), a]);
    }
    (size.width(), size.height(), out)
}

fn difference(a: &[u8], b: &[u8], tolerance: u8) -> f64 {
    let differing = a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .filter(|(p, q)| {
            p.iter()
                .zip(q.iter())
                .any(|(x, y)| x.abs_diff(*y) > tolerance)
        })
        .count();
    differing as f64 / (a.len() / 4) as f64
}

fn compare(config: &RenderConfig, label: &str, max_differing: f64) {
    let renderer = Renderer::hermetic();
    let request = RenderRequest::new(SOURCE, config);

    let raster = renderer.render_raster(&request).expect("raster render");
    let (svg, dropped) = renderer.render_svg(&request).expect("svg render");
    assert!(!dropped, "{label}: nothing should have been dropped");

    let (w, h, svg_pixels) = rasterize_svg(&svg);
    assert_eq!(
        (w, h),
        (raster.width, raster.height),
        "{label}: SVG and PNG must agree on dimensions"
    );

    let diff = difference(&raster.pixels, &svg_pixels, 24);
    assert!(
        diff <= max_differing,
        "{label}: {:.2}% of pixels differ (limit {:.2}%)",
        diff * 100.0,
        max_differing * 100.0
    );
}

#[test]
fn svg_matches_raster_for_the_default_render() {
    compare(&RenderConfig::default(), "default", 0.05);
}

#[test]
fn svg_matches_raster_with_gutter_and_border() {
    let mut config = RenderConfig::default();
    config.gutter.enabled = true;
    config.gutter.separator = true;
    config.window.border = true;
    config.window.title = Some("Point.swift".into());
    compare(&config, "gutter", 0.05);
}

#[test]
fn svg_matches_raster_for_gradients() {
    let mut config = RenderConfig::default();
    config.background = Background::Linear {
        stops: vec![
            GradientStop::new(0.0, "#1e3a8a".parse().unwrap()),
            GradientStop::new(1.0, "#701a75".parse().unwrap()),
        ],
        angle: 135.0,
    };
    config.shadow = None;
    compare(&config, "linear gradient", 0.05);

    config.background = Background::Radial {
        stops: vec![
            GradientStop::new(0.0, "#4c1d95".parse().unwrap()),
            GradientStop::new(1.0, "#0f172a".parse().unwrap()),
        ],
    };
    compare(&config, "radial gradient", 0.05);
}

#[test]
fn svg_matches_raster_without_a_titlebar() {
    let mut config = RenderConfig::default();
    config.window.titlebar_height = None;
    config.shadow = None;
    compare(&config, "no titlebar", 0.05);
}

#[test]
fn svg_reports_dropped_image_backgrounds() {
    let dir = std::env::temp_dir().join("snapcode-svg-parity");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bg.png");
    let pixels: Vec<u8> = (0..16 * 16).flat_map(|_| [30u8, 40, 60, 255]).collect();
    let raster = snapcode_core::backend::raster::Raster {
        width: 16,
        height: 16,
        pixels,
    };
    std::fs::write(&path, snapcode_core::encode::to_png(&raster, 72).unwrap()).unwrap();

    let mut config = RenderConfig::default();
    config.background = Background::Image {
        path,
        fit: Default::default(),
        blur: 0.0,
        darken: 0.0,
    };
    let renderer = Renderer::hermetic();
    let (_, dropped) = renderer
        .render_svg(&RenderRequest::new(SOURCE, &config))
        .unwrap();
    assert!(dropped, "an image background must be reported as dropped");
}
