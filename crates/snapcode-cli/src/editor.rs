// SPDX-License-Identifier: MIT
//! Handing a snippet to $EDITOR and taking the edited text back.

use std::process::Command;

use anyhow::{bail, Context, Result};

pub fn edit(seed: &str, extension: &str) -> Result<String> {
    let (program, args) = editor_command();
    edit_with(&program, &args, seed, extension)
}

fn edit_with(program: &str, args: &[String], seed: &str, extension: &str) -> Result<String> {
    let file = tempfile::Builder::new()
        .prefix("snapcode-")
        .suffix(&format!(".{extension}"))
        .tempfile()
        .context("failed to create a scratch file for the editor")?;
    std::fs::write(file.path(), seed)
        .with_context(|| format!("failed to seed {}", file.path().display()))?;

    let status = Command::new(program)
        .args(args)
        .arg(file.path())
        .status()
        .with_context(|| format!("failed to run {program}"))?;
    if !status.success() {
        bail!("{program} exited with {status}; the snippet was left unchanged");
    }

    std::fs::read_to_string(file.path())
        .with_context(|| format!("failed to read back {}", file.path().display()))
}

fn editor_command() -> (String, Vec<String>) {
    let configured = ["VISUAL", "EDITOR"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.trim().is_empty());

    match configured {
        Some(value) => split(&value),
        None => (fallback().to_string(), Vec::new()),
    }
}

fn split(value: &str) -> (String, Vec<String>) {
    let trimmed = value.trim();
    if std::path::Path::new(trimmed).is_file() {
        return (trimmed.to_string(), Vec::new());
    }

    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quote: Option<char> = None;

    for ch in trimmed.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => word.push(ch),
            None if ch == '\'' || ch == '"' => {
                quote = Some(ch);
                started = true;
            }
            None if ch.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            None => {
                word.push(ch);
                started = true;
            }
        }
    }
    if started {
        words.push(word);
    }

    let mut words = words.into_iter();
    let program = words.next().unwrap_or_else(|| fallback().to_string());
    (program, words.collect())
}

fn fallback() -> &'static str {
    if cfg!(windows) {
        "notepad"
    } else {
        "vi"
    }
}

pub fn extension_for(path: Option<&std::path::Path>, language: Option<&str>) -> String {
    path.and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .map(str::to_string)
        .or_else(|| language.map(str::to_string))
        .unwrap_or_else(|| "txt".to_string())
}

pub fn scratch_extension(
    renderer: &snapcode_core::Renderer,
    path: Option<&std::path::Path>,
    language: &str,
) -> String {
    extension_for(path, renderer.highlighter().primary_extension(language))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_editor_has_no_arguments() {
        assert_eq!(split("nvim"), ("nvim".to_string(), Vec::new()));
    }

    #[test]
    fn editor_arguments_are_kept() {
        assert_eq!(
            split("code -w"),
            ("code".to_string(), vec!["-w".to_string()])
        );
        assert_eq!(
            split("emacsclient -nw -c"),
            (
                "emacsclient".to_string(),
                vec!["-nw".to_string(), "-c".to_string()]
            )
        );
    }

    #[test]
    fn a_quoted_editor_path_survives_its_spaces() {
        assert_eq!(
            split(r#""/Applications/Sublime Text.app/bin/subl" -w"#),
            (
                "/Applications/Sublime Text.app/bin/subl".to_string(),
                vec!["-w".to_string()]
            )
        );
    }

    #[test]
    fn an_unquoted_editor_that_is_itself_a_file_is_not_split() {
        let file = tempfile::Builder::new()
            .prefix("snapcode editor ")
            .tempfile()
            .unwrap();
        let path = file.path().to_string_lossy().to_string();
        assert!(path.contains(' '), "{path}");
        assert_eq!(split(&path), (path.clone(), Vec::new()));
    }

    #[cfg(unix)]
    #[test]
    fn the_editors_changes_come_back() {
        let args: Vec<String> = ["-c", r#"printf 'let y = 2\n' >> "$1""#, "sh"]
            .iter()
            .map(|a| a.to_string())
            .collect();
        let text = edit_with("sh", &args, "let x = 1\n", "swift").unwrap();
        assert_eq!(text, "let x = 1\nlet y = 2\n");
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_editor_is_reported_rather_than_wiping_the_snippet() {
        let args: Vec<String> = ["-c", "exit 3", "sh"]
            .iter()
            .map(|a| a.to_string())
            .collect();
        let error = edit_with("sh", &args, "let x = 1\n", "swift").unwrap_err();
        assert!(error.to_string().contains("left unchanged"), "{error}");
    }

    #[test]
    fn a_missing_editor_is_reported_by_name() {
        let error = edit_with("snapcode-no-such-editor", &[], "", "txt").unwrap_err();
        assert!(
            error.to_string().contains("snapcode-no-such-editor"),
            "{error}"
        );
    }

    #[test]
    fn an_extension_comes_from_the_path_then_the_language() {
        let path = std::path::PathBuf::from("src/Feed.swift");
        assert_eq!(extension_for(Some(&path), Some("rust")), "swift");
        assert_eq!(extension_for(None, Some("rust")), "rust");
        assert_eq!(extension_for(None, None), "txt");
        assert_eq!(
            extension_for(Some(std::path::Path::new("Makefile")), None),
            "txt"
        );
    }
}
