// SPDX-License-Identifier: MIT
//! Reading source in and writing images out: files, stdin/stdout, clipboard.

use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

#[derive(Debug, Clone)]
pub struct Input {
    pub source: String,
    pub path: Option<PathBuf>,
}

pub fn read_input(spec: Option<&str>) -> Result<Input> {
    match spec {
        Some("-") => Ok(Input {
            source: read_stdin()?,
            path: None,
        }),
        Some(path) => {
            let path = PathBuf::from(path);
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            Ok(Input {
                source,
                path: Some(path),
            })
        }
        None => {
            if !std::io::stdin().is_terminal() {
                return Ok(Input {
                    source: read_stdin()?,
                    path: None,
                });
            }
            Ok(Input {
                source: read_clipboard_text()?,
                path: None,
            })
        }
    }
}

fn read_stdin() -> Result<String> {
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .context("failed to read stdin")?;
    Ok(buf)
}

fn read_clipboard_text() -> Result<String> {
    let mut clipboard = arboard::Clipboard::new().context("failed to open the clipboard")?;
    let text = clipboard
        .get_text()
        .context("the clipboard does not contain text")?;
    if text.trim().is_empty() {
        bail!("the clipboard is empty; pass a file, or `-` to read stdin");
    }
    Ok(text)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    File(PathBuf),
    Stdout,
}

pub fn resolve_destination(
    output: Option<&str>,
    input_path: Option<&Path>,
    extension: &str,
) -> Destination {
    match output {
        Some("-") => Destination::Stdout,
        Some(path) => Destination::File(PathBuf::from(path)),
        None => {
            let stem = input_path
                .and_then(|p| p.file_stem())
                .and_then(|s| s.to_str())
                .unwrap_or("snippet");
            Destination::File(PathBuf::from(format!("{stem}.{extension}")))
        }
    }
}

pub fn write_output(destination: &Destination, bytes: &[u8]) -> Result<()> {
    match destination {
        Destination::File(path) => {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            std::fs::write(path, bytes)
                .with_context(|| format!("failed to write {}", path.display()))
        }
        Destination::Stdout => {
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            handle.write_all(bytes).context("failed to write stdout")?;
            handle.flush().context("failed to flush stdout")
        }
    }
}

pub fn copy_image_to_clipboard(width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new().context("failed to open the clipboard")?;
    clipboard
        .set_image(arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: std::borrow::Cow::Borrowed(rgba),
        })
        .context("failed to put the image on the clipboard")
}

pub fn copy_text_to_clipboard(text: &str) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new().context("failed to open the clipboard")?;
    clipboard
        .set_text(text.to_string())
        .context("failed to put text on the clipboard")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdout_is_requested_with_a_dash() {
        assert_eq!(
            resolve_destination(Some("-"), None, "png"),
            Destination::Stdout
        );
    }

    #[test]
    fn explicit_output_paths_are_used_verbatim() {
        assert_eq!(
            resolve_destination(Some("out/img.png"), Some(Path::new("a.swift")), "png"),
            Destination::File(PathBuf::from("out/img.png"))
        );
    }

    #[test]
    fn output_defaults_to_the_input_stem() {
        assert_eq!(
            resolve_destination(None, Some(Path::new("src/Feed.swift")), "png"),
            Destination::File(PathBuf::from("Feed.png"))
        );
        assert_eq!(
            resolve_destination(None, Some(Path::new("src/Feed.swift")), "svg"),
            Destination::File(PathBuf::from("Feed.svg"))
        );
    }

    #[test]
    fn output_falls_back_to_a_generic_name_without_an_input_path() {
        assert_eq!(
            resolve_destination(None, None, "png"),
            Destination::File(PathBuf::from("snippet.png"))
        );
    }

    #[test]
    fn writing_creates_missing_parent_directories() {
        let dir = std::env::temp_dir().join("snapcode-io-test").join("nested");
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
        let path = dir.join("out.png");

        write_output(&Destination::File(path.clone()), b"hello").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"hello");

        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn reading_a_missing_file_is_an_error_naming_the_path() {
        let err = read_input(Some("/nonexistent/snapcode/x.swift")).unwrap_err();
        assert!(
            err.to_string().contains("/nonexistent/snapcode/x.swift"),
            "{err}"
        );
    }

    #[test]
    fn reads_a_file_and_keeps_its_path() {
        let path = std::env::temp_dir().join("snapcode-io-read.swift");
        std::fs::write(&path, "let x = 1\n").unwrap();
        let input = read_input(path.to_str()).unwrap();
        assert_eq!(input.source, "let x = 1\n");
        assert_eq!(input.path.as_deref(), Some(path.as_path()));
        let _ = std::fs::remove_file(&path);
    }
}
