// SPDX-License-Identifier: MIT
//! End-to-end tests of the binary: real arguments, real files, real exit codes.

use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

const SWIFT: &str = "struct Point {\n    let x: Int\n    let y: Int\n}\n";

fn snapcode() -> Command {
    let mut command = Command::cargo_bin("snapcode").expect("the binary should build");
    command.arg("--no-config");
    command
}

fn workspace() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("temp dir");
    let source = dir.path().join("Point.swift");
    std::fs::write(&source, SWIFT).expect("write source");
    (dir, source)
}

fn png_size(path: &Path) -> (u32, u32) {
    let bytes = std::fs::read(path).expect("read png");
    assert_eq!(
        &bytes[..8],
        b"\x89PNG\r\n\x1a\n",
        "not a PNG: {}",
        path.display()
    );
    let read = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
    (read(16), read(20))
}

#[test]
fn renders_a_file_to_a_png() {
    let (dir, source) = workspace();
    let output = dir.path().join("out.png");

    snapcode()
        .arg(&source)
        .arg("-o")
        .arg(&output)
        .assert()
        .success()
        .stderr(predicate::str::contains("wrote"));

    let (width, height) = png_size(&output);
    assert!(
        width > 100 && height > 100,
        "suspiciously small: {width}x{height}"
    );
}

#[test]
fn output_defaults_to_the_input_name() {
    let (dir, source) = workspace();

    snapcode()
        .current_dir(dir.path())
        .arg(source.file_name().unwrap())
        .assert()
        .success();

    assert!(dir.path().join("Point.png").exists(), "expected Point.png");
}

#[test]
fn reads_stdin_and_writes_stdout() {
    let output = snapcode()
        .arg("-")
        .arg("--lang")
        .arg("swift")
        .arg("-o")
        .arg("-")
        .write_stdin(SWIFT)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(&output[..8], b"\x89PNG\r\n\x1a\n");
    assert!(
        output.len() > 1000,
        "stdout was only {} bytes",
        output.len()
    );
}

#[test]
fn scale_changes_the_output_dimensions() {
    let (dir, source) = workspace();

    let render_at = |scale: &str, name: &str| {
        let output = dir.path().join(name);
        snapcode()
            .arg(&source)
            .args(["--scale", scale, "-o"])
            .arg(&output)
            .assert()
            .success();
        png_size(&output)
    };

    let (w1, h1) = render_at("1", "one.png");
    let (w2, h2) = render_at("2", "two.png");
    assert!(w2 > w1 && h2 > h1, "{w2}x{h2} should exceed {w1}x{h1}");
    assert!(
        (w2 as f32 / w1 as f32 - 2.0).abs() < 0.1,
        "width should roughly double"
    );
}

#[test]
fn svg_is_chosen_from_the_output_extension() {
    let (dir, source) = workspace();
    let output = dir.path().join("out.svg");

    snapcode()
        .arg(&source)
        .arg("-o")
        .arg(&output)
        .assert()
        .success();

    let svg = std::fs::read_to_string(&output).unwrap();
    assert!(
        svg.starts_with("<svg "),
        "not an SVG: {}",
        &svg[..40.min(svg.len())]
    );
    assert!(svg.ends_with("</svg>"));
    assert!(
        !svg.contains("font-family"),
        "SVG should not depend on a font"
    );
    assert!(svg.contains("<path"), "SVG should contain glyph paths");
}

#[test]
fn line_numbers_widen_the_image() {
    let (dir, source) = workspace();

    let render = |name: &str, extra: &[&str]| {
        let output = dir.path().join(name);
        snapcode()
            .arg(&source)
            .args(extra)
            .arg("-o")
            .arg(&output)
            .assert()
            .success();
        png_size(&output)
    };

    let (plain, _) = render("plain.png", &[]);
    let (numbered, _) = render("numbered.png", &["--line-numbers"]);
    assert!(numbered > plain, "{numbered} should exceed {plain}");
}

#[test]
fn a_missing_input_file_fails_with_a_useful_message() {
    snapcode()
        .arg("/nonexistent/snapcode/nope.swift")
        .assert()
        .failure()
        .stderr(predicate::str::contains("nope.swift"));
}

#[test]
fn an_unknown_theme_lists_the_valid_ones() {
    let (_dir, source) = workspace();
    snapcode()
        .arg(&source)
        .args(["--theme", "does-not-exist"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does-not-exist").and(predicate::str::contains("warm")));
}

#[test]
fn an_unknown_language_is_reported_rather_than_ignored() {
    let (_dir, source) = workspace();
    snapcode()
        .arg(&source)
        .args(["--lang", "swfit"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("swfit"));
}

#[test]
fn themes_and_languages_are_listable() {
    snapcode()
        .arg("themes")
        .assert()
        .success()
        .stdout(predicate::str::contains("warm").and(predicate::str::contains("midnight")));

    snapcode()
        .arg("languages")
        .assert()
        .success()
        .stdout(predicate::str::contains("Swift").and(predicate::str::contains("Rust")));
}

#[test]
fn config_print_reflects_the_flags() {
    snapcode()
        .args(["config", "print", "--scale", "3", "--theme", "midnight"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("scale = 3.0")
                .and(predicate::str::contains("theme = \"midnight\"")),
        );
}

#[test]
fn a_project_config_is_picked_up_from_the_working_directory() {
    let (dir, source) = workspace();
    std::fs::write(dir.path().join("snapcode.toml"), "scale = 1.0\n").unwrap();

    Command::cargo_bin("snapcode")
        .unwrap()
        .current_dir(dir.path())
        .arg(source.file_name().unwrap())
        .args(["-o", "out.png"])
        .assert()
        .success();
    let from_config = png_size(&dir.path().join("out.png"));

    Command::cargo_bin("snapcode")
        .unwrap()
        .current_dir(dir.path())
        .arg(source.file_name().unwrap())
        .args(["--scale", "2", "-o", "flag.png"])
        .assert()
        .success();
    let from_flag = png_size(&dir.path().join("flag.png"));

    assert!(
        from_flag.0 > from_config.0,
        "the flag should override the project config: {from_flag:?} vs {from_config:?}"
    );
}

#[test]
fn completions_are_generated_for_common_shells() {
    for shell in ["bash", "zsh", "fish"] {
        snapcode()
            .args(["completions", shell])
            .assert()
            .success()
            .stdout(predicate::str::contains("snapcode"));
    }
}

#[test]
fn the_tui_refuses_to_start_without_a_terminal() {
    let (_dir, source) = workspace();
    snapcode()
        .arg("tui")
        .arg(&source)
        .assert()
        .failure()
        .stderr(predicate::str::contains("terminal"));
}
