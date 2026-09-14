// SPDX-License-Identifier: MIT
//! The interactive TUI: a settings form beside a live preview of the real image.

mod fields;
mod palette;
mod picker;
mod preview;

use std::io::{IsTerminal, Stdout};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use ratatui_image::picker::Picker as ImagePicker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};
use snapcode_core::config::RenderConfig;
use snapcode_core::{RenderRequest, Renderer};

use crate::cli::RenderArgs;
use crate::io::Input;
use crate::settings;
use fields::{Context as FieldContext, Field, Section, FIELDS};
use palette::{to_color, Palette};
use picker::{Picker, AUTOMATIC};

const PREVIEW_SCALE: f32 = 1.0;

const STATUS_TTL: Duration = Duration::from_secs(4);

type Backend = ratatui::backend::CrosstermBackend<Stdout>;

pub fn run(
    config: RenderConfig,
    renderer: Renderer,
    input: Option<Input>,
    args: &RenderArgs,
) -> Result<()> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        anyhow::bail!(
            "the TUI needs an interactive terminal; \
             pipe input with `snapcode tui -` only from a terminal, \
             or use the non-interactive command instead"
        );
    }

    let (picker, mode) = preview::build_picker();

    let mut terminal = enter_terminal()?;
    let mut app = App::new(config, renderer, input, args.clone(), picker);
    if mode.degraded {
        app.set_status(
            "no terminal graphics detected; showing a half-block preview              (set SNAPCODE_IMAGE_PROTOCOL to override)"
                .into(),
        );
    }
    let result = app.event_loop(&mut terminal);

    restore_terminal(&mut terminal)?;

    if let Some(command) = app.print_on_exit.take() {
        println!("{command}");
    }
    result
}

fn enter_screen() -> Result<()> {
    enable_raw_mode().context("failed to enable raw mode")?;
    execute!(std::io::stdout(), EnterAlternateScreen)
        .context("failed to enter the alternate screen")
}

fn leave_screen() {
    disable_raw_mode().ok();
    execute!(std::io::stdout(), LeaveAlternateScreen).ok();
}

fn enter_terminal() -> Result<Terminal<Backend>> {
    enter_screen()?;
    Terminal::new(ratatui::backend::CrosstermBackend::new(std::io::stdout()))
        .context("failed to initialize the terminal")
}

fn restore_terminal(terminal: &mut Terminal<Backend>) -> Result<()> {
    leave_screen();
    terminal.show_cursor().ok();
    Ok(())
}

struct App {
    config: RenderConfig,
    renderer: Renderer,
    input: Input,
    args: RenderArgs,
    image_picker: ImagePicker,

    chrome_themes: Vec<String>,
    syntax_themes: Vec<String>,
    languages: Vec<String>,
    detected_language: String,
    palette: Palette,
    picker: Option<Picker>,

    selected: usize,
    scroll: usize,
    dirty: bool,
    preview: Option<StatefulProtocol>,
    export_size: Option<(u32, u32)>,
    error: Option<String>,
    status: Option<(String, Instant)>,
    show_help: bool,
    pending_edit: bool,
    edited: bool,
    quit: bool,
    print_on_exit: Option<String>,
}

impl App {
    fn new(
        config: RenderConfig,
        renderer: Renderer,
        input: Option<Input>,
        args: RenderArgs,
        image_picker: ImagePicker,
    ) -> Self {
        let chrome_themes = renderer
            .themes()
            .chrome_names()
            .into_iter()
            .map(str::to_string)
            .collect();
        let syntax_themes: Vec<String> = renderer
            .themes()
            .syntax_names()
            .into_iter()
            .map(str::to_string)
            .collect();
        let languages = renderer
            .highlighter()
            .language_names()
            .into_iter()
            .map(str::to_string)
            .collect();

        let palette = build_palette(&renderer, &config);

        Self {
            config,
            renderer,
            input: input.unwrap_or_else(|| Input {
                source: String::new(),
                path: None,
            }),
            args,
            image_picker,
            chrome_themes,
            syntax_themes,
            languages,
            detected_language: String::new(),
            palette,
            picker: None,
            selected: 0,
            scroll: 0,
            dirty: true,
            preview: None,
            export_size: None,
            error: None,
            status: None,
            show_help: false,
            pending_edit: false,
            edited: false,
            quit: false,
            print_on_exit: None,
        }
    }

    fn event_loop(&mut self, terminal: &mut Terminal<Backend>) -> Result<()> {
        while !self.quit {
            if self.dirty && !event::poll(Duration::ZERO)? {
                self.refresh_preview();
                self.dirty = false;
            }

            terminal.draw(|frame| self.draw(frame))?;

            if event::poll(Duration::from_millis(200))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key);
                    }
                }
            }
            if self.pending_edit {
                self.pending_edit = false;
                self.edit_snippet(terminal)?;
            }
            if let Some((_, at)) = &self.status {
                if at.elapsed() > STATUS_TTL {
                    self.status = None;
                }
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.quit = true;
            return;
        }
        if self.show_help {
            self.show_help = false;
            return;
        }
        if self.picker.is_some() {
            self.handle_picker_key(key);
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Home | KeyCode::Char('g') => self.selected = 0,
            KeyCode::End | KeyCode::Char('G') => self.selected = FIELDS.len() - 1,
            KeyCode::Right | KeyCode::Char('l') => self.adjust(1),
            KeyCode::Left | KeyCode::Char('h') => self.adjust(-1),
            KeyCode::Char(' ') | KeyCode::Enter => {
                if FIELDS[self.selected].is_picker() {
                    self.open_picker();
                } else {
                    self.adjust(1);
                }
            }
            KeyCode::Char('/') => self.open_picker(),
            KeyCode::Char('e') => self.export(),
            KeyCode::Char('c') => self.copy(),
            KeyCode::Char('s') => self.save_config(),
            KeyCode::Char('p') => self.print_command(),
            KeyCode::Char('r') => self.reload_input(),
            KeyCode::Char('i') => self.pending_edit = true,
            _ => {}
        }
    }

    fn open_picker(&mut self) {
        let field = FIELDS[self.selected];
        let (title, items, current) = match field {
            Field::Theme => (
                "Theme",
                self.chrome_themes.clone(),
                Some(self.config.theme.clone()),
            ),
            Field::SyntaxTheme => (
                "Syntax theme",
                self.syntax_themes.clone(),
                Some(self.config.syntax_theme.clone()),
            ),
            Field::Language => {
                let mut items = vec![AUTOMATIC.to_string()];
                items.extend(self.languages.iter().cloned());
                let current = self
                    .config
                    .code
                    .language
                    .clone()
                    .unwrap_or_else(|| AUTOMATIC.to_string());
                ("Language", items, Some(current))
            }
            _ => return,
        };
        self.picker = Some(Picker::new(field, title, items, current.as_deref()));
    }

    fn handle_picker_key(&mut self, key: KeyEvent) {
        let Some(picker) = self.picker.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc => self.picker = None,
            KeyCode::Enter => {
                let chosen = picker.current().map(str::to_string);
                let field = picker.field;
                self.picker = None;
                if let Some(chosen) = chosen {
                    self.apply_choice(field, chosen);
                }
            }
            KeyCode::Down => picker.move_selection(1),
            KeyCode::Up => picker.move_selection(-1),
            KeyCode::Backspace => picker.pop(),
            KeyCode::Char(ch) => picker.push(ch),
            _ => {}
        }
    }

    fn apply_choice(&mut self, field: Field, chosen: String) {
        match field {
            Field::Theme => self.config.theme = chosen,
            Field::SyntaxTheme => self.config.syntax_theme = chosen,
            Field::Language => {
                self.config.code.language = (chosen != AUTOMATIC).then_some(chosen);
            }
            _ => return,
        }
        self.dirty = true;
    }

    fn context(&self) -> FieldContext<'_> {
        FieldContext {
            detected_language: &self.detected_language,
        }
    }

    fn move_selection(&mut self, delta: i32) {
        let len = FIELDS.len() as i32;
        self.selected = (self.selected as i32 + delta).rem_euclid(len) as usize;
    }

    fn adjust(&mut self, delta: i32) {
        FIELDS[self.selected].adjust(
            &mut self.config,
            delta,
            &self.chrome_themes,
            &self.syntax_themes,
        );
        self.dirty = true;
    }

    fn refresh_palette(&mut self) {
        self.palette = build_palette(&self.renderer, &self.config);
    }

    fn is_empty(&self) -> bool {
        self.input.source.trim().is_empty()
    }

    fn refresh_preview(&mut self) {
        self.refresh_palette();

        if self.is_empty() {
            self.error = None;
            self.preview = None;
            self.export_size = None;
            return;
        }

        let mut preview_config = self.config.clone();
        preview_config.scale = PREVIEW_SCALE;

        self.detected_language = self
            .renderer
            .highlighter()
            .resolve(
                self.config.code.language.as_deref(),
                self.input.path.as_deref(),
                &self.input.source,
            )
            .map(|syntax| syntax.name.clone())
            .unwrap_or_else(|_| "unknown".to_string());

        let raster = match self.render_with(&preview_config) {
            Ok(raster) => raster,
            Err(error) => {
                self.error = Some(format!("{error:#}"));
                self.preview = None;
                self.export_size = None;
                return;
            }
        };
        self.error = None;

        let factor = self.config.scale / PREVIEW_SCALE;
        self.export_size = Some((
            (raster.width as f32 * factor).round() as u32,
            (raster.height as f32 * factor).round() as u32,
        ));

        let Some(buffer) = image::RgbaImage::from_raw(raster.width, raster.height, raster.pixels)
        else {
            self.error = Some("the preview image was the wrong size".into());
            return;
        };
        self.preview = Some(
            self.image_picker
                .new_resize_protocol(image::DynamicImage::ImageRgba8(buffer)),
        );
    }

    fn render_with(&self, config: &RenderConfig) -> Result<snapcode_core::backend::raster::Raster> {
        let mut request = RenderRequest::new(&self.input.source, config);
        if let Some(path) = &self.input.path {
            request = request.with_path(path.clone());
        }
        Ok(self.renderer.render_raster(&request)?)
    }

    fn scratch_language(&self) -> &str {
        self.config
            .code
            .language
            .as_deref()
            .filter(|l| !l.trim().is_empty())
            .unwrap_or(&self.detected_language)
    }

    fn edit_snippet(&mut self, terminal: &mut Terminal<Backend>) -> Result<()> {
        let extension = crate::editor::scratch_extension(
            &self.renderer,
            self.input.path.as_deref(),
            self.scratch_language(),
        );

        leave_screen();
        let edited = crate::editor::edit(&self.input.source, &extension);
        enter_screen()?;
        terminal.clear()?;

        self.preview = None;
        self.dirty = true;

        match edited {
            Ok(source) => {
                if source == self.input.source {
                    self.set_status(
                        "the snippet is unchanged; if your editor detaches, \
                         set EDITOR to wait (for example `code --wait`)"
                            .into(),
                    );
                    return Ok(());
                }
                let empty = source.trim().is_empty();
                self.input.source = source;
                self.edited = true;
                self.set_status(if empty {
                    "the snippet is empty; press i to write one".into()
                } else {
                    "snippet updated".into()
                });
            }
            Err(error) => self.set_status(format!("edit failed: {error:#}")),
        }
        Ok(())
    }

    fn export(&mut self) {
        if self.is_empty() {
            self.set_status("nothing to render yet; press i to add a snippet".into());
            return;
        }
        match self.write_export() {
            Ok(message) => self.set_status(message),
            Err(error) => self.set_status(format!("export failed: {error:#}")),
        }
    }

    fn write_export(&self) -> Result<String> {
        let raster = self.render_with(&self.config)?;
        let png = snapcode_core::encode::to_png(&raster, self.config.dpi)?;
        let destination = crate::io::resolve_destination(
            self.args.output.as_deref(),
            self.input.path.as_deref(),
            "png",
        );
        crate::io::write_output(&destination, &png)?;
        Ok(match &destination {
            crate::io::Destination::File(path) => format!(
                "wrote {} ({}x{})",
                path.display(),
                raster.width,
                raster.height
            ),
            crate::io::Destination::Stdout => "wrote to stdout".to_string(),
        })
    }

    fn copy(&mut self) {
        if self.is_empty() {
            self.set_status("nothing to render yet; press i to add a snippet".into());
            return;
        }
        let result = self.render_with(&self.config).and_then(|raster| {
            crate::io::copy_image_to_clipboard(raster.width, raster.height, &raster.pixels)
        });
        match result {
            Ok(()) => self.set_status("copied to the clipboard".into()),
            Err(error) => self.set_status(format!("copy failed: {error:#}")),
        }
    }

    fn save_config(&mut self) {
        match settings::save(&self.config) {
            Ok(path) => self.set_status(format!("saved {}", path.display())),
            Err(error) => self.set_status(format!("save failed: {error:#}")),
        }
    }

    fn print_command(&mut self) {
        self.print_on_exit = Some(self.equivalent_command());
        self.quit = true;
    }

    fn equivalent_command(&self) -> String {
        let mut parts = vec!["snapcode".to_string()];
        let mut unrepresentable: Vec<&str> = Vec::new();
        match &self.input.path {
            Some(path) if !self.edited => parts.push(shell_quote(&path.to_string_lossy())),
            _ => parts.push("-".to_string()),
        }

        let default = RenderConfig::default();
        let config = &self.config;
        fn flag(parts: &mut Vec<String>, name: &str, value: String) {
            parts.push(format!("{name} {value}"));
        }

        if config.theme != default.theme {
            flag(&mut parts, "--theme", shell_quote(&config.theme));
        }
        if config.syntax_theme != default.syntax_theme {
            flag(
                &mut parts,
                "--syntax-theme",
                shell_quote(&config.syntax_theme),
            );
        }
        if config.scale != default.scale {
            flag(&mut parts, "--scale", format!("{}", config.scale));
        }
        if config.dpi != default.dpi {
            flag(&mut parts, "--dpi", format!("{}", config.dpi));
        }
        if config.font.size != default.font.size {
            flag(&mut parts, "--font-size", format!("{}", config.font.size));
        }
        if config.font.line_height != default.font.line_height {
            flag(
                &mut parts,
                "--line-height",
                format!("{}", config.font.line_height),
            );
        }
        if config.window.padding != default.window.padding {
            flag(
                &mut parts,
                "--padding",
                format!("{}", config.window.padding),
            );
        }
        if config.window.margin != default.window.margin {
            flag(&mut parts, "--margin", format!("{}", config.window.margin));
        }
        if config.window.radius != default.window.radius {
            flag(&mut parts, "--radius", format!("{}", config.window.radius));
        }
        if config.code.tab_width != default.code.tab_width {
            flag(
                &mut parts,
                "--tab-width",
                format!("{}", config.code.tab_width),
            );
        }
        if let Some(shadow) = &config.shadow {
            if Some(shadow.blur) != default.shadow.as_ref().map(|s| s.blur) {
                flag(&mut parts, "--shadow-blur", format!("{}", shadow.blur));
            }
        }

        match fields::background_name(&config.background) {
            "theme" => {}
            name @ ("custom" | "image") => unrepresentable.push(name),
            "gradient" => flag(&mut parts, "--bg", "linear:135:#1e3a8a,#701a75".into()),
            "sunset" => flag(&mut parts, "--bg", "radial:#f97316,#7c2d12".into()),
            name => flag(&mut parts, "--bg", name.to_string()),
        }

        if config.font.ligatures {
            parts.push("--ligatures".into());
        }
        if config.window.border {
            parts.push("--border".into());
        }
        if config.window.titlebar_height.is_none() {
            parts.push("--no-titlebar".into());
        }
        if let Some(height) = config.window.titlebar_height {
            if Some(height) != default.window.titlebar_height {
                unrepresentable.push("titlebar height");
            }
        }
        if config.shadow.is_none() {
            parts.push("--no-shadow".into());
        }
        if config.gutter.enabled {
            parts.push("--line-numbers".into());
        }
        if config.gutter.separator {
            parts.push("--gutter-separator".into());
        }
        if config.code.diff {
            parts.push("--diff".into());
        }
        if config.window.traffic_lights != default.window.traffic_lights {
            flag(
                &mut parts,
                "--traffic-lights",
                Field::TrafficLights.value(config, &self.context()),
            );
        }
        let mut command = parts.join(" ");
        if !unrepresentable.is_empty() {
            command = format!(
                "{command}\n# not expressible as flags, kept only in the config: {}",
                unrepresentable.join(", ")
            );
        }
        if parts.get(1).is_some_and(|operand| operand == "-") {
            command = format!(
                "{command}\n# reads the snippet on stdin: this one was written here, not in a file"
            );
        }
        command
    }

    fn reload_input(&mut self) {
        let Some(path) = self.input.path.clone() else {
            self.set_status("nothing to reload: the input did not come from a file".into());
            return;
        };
        match std::fs::read_to_string(&path) {
            Ok(source) => {
                let discarded = self.edited && source != self.input.source;
                self.input.source = source;
                self.edited = false;
                self.dirty = true;
                self.set_status(if discarded {
                    format!("reloaded {}, discarding your edits", path.display())
                } else {
                    format!("reloaded {}", path.display())
                });
            }
            Err(error) => self.set_status(format!("reload failed: {error}")),
        }
    }

    fn set_status(&mut self, message: String) {
        self.status = Some((message, Instant::now()));
    }

    fn draw(&mut self, frame: &mut Frame) {
        frame.render_widget(
            Block::default().style(Style::default().bg(to_color(self.palette.background))),
            frame.area(),
        );

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(1)])
            .split(frame.area());

        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(32), Constraint::Min(20)])
            .split(chunks[0]);

        self.draw_form(frame, columns[0]);
        self.draw_preview(frame, columns[1]);
        self.draw_status(frame, chunks[1]);

        if self.picker.is_some() {
            self.draw_picker(frame);
        }
        if self.show_help {
            draw_help(frame, &self.palette);
        }
    }

    fn draw_form(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(self.palette.border())
            .title(Span::styled(" Settings ", self.palette.title()))
            .style(Style::default().bg(to_color(self.palette.background)));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let rows = self.form_rows();
        let visible = inner.height as usize;
        let cursor = rows
            .iter()
            .position(|row| matches!(row, FormRow::Field(index) if *index == self.selected))
            .unwrap_or(0);
        if visible > 0 {
            if cursor < self.scroll {
                self.scroll = cursor;
            } else if cursor >= self.scroll + visible {
                self.scroll = cursor + 1 - visible;
            }
            self.scroll = self.scroll.min(rows.len().saturating_sub(visible));
        }

        let context = self.context();
        let value_column = 15usize;
        let lines: Vec<Line> = rows
            .iter()
            .skip(self.scroll)
            .take(visible)
            .map(|row| match row {
                FormRow::Heading(section) => Line::from(Span::styled(
                    format!(
                        " {:<width$}",
                        section.label(),
                        width = inner.width.saturating_sub(1) as usize
                    ),
                    Style::default()
                        .fg(to_color(self.palette.muted))
                        .bg(to_color(self.palette.surface))
                        .add_modifier(Modifier::BOLD),
                )),
                FormRow::Field(index) => {
                    let field = FIELDS[*index];
                    let selected = *index == self.selected;
                    let value = field.value(&self.config, &context);

                    if selected {
                        let text = format!(
                            " {:<value_column$}{:<width$}",
                            field.label(),
                            value,
                            width = (inner.width as usize).saturating_sub(value_column + 1),
                        );
                        Line::from(Span::styled(text, self.palette.selection()))
                    } else {
                        Line::from(vec![
                            Span::styled(
                                format!(" {:<value_column$}", field.label()),
                                self.palette.label(),
                            ),
                            Span::styled(value, self.value_style(field)),
                        ])
                    }
                }
            })
            .collect();

        frame.render_widget(
            Paragraph::new(lines).style(Style::default().bg(to_color(self.palette.background))),
            inner,
        );
    }

    fn value_style(&self, field: Field) -> Style {
        let context = self.context();
        match field.value(&self.config, &context).as_str() {
            "on" => self.palette.fg(self.palette.positive),
            "off" | "-" => self.palette.fg(self.palette.muted),
            _ => self.palette.base(),
        }
    }

    fn form_rows(&self) -> Vec<FormRow> {
        let mut rows = Vec::with_capacity(FIELDS.len() + 4);
        let mut current: Option<Section> = None;
        for (index, field) in FIELDS.iter().enumerate() {
            let section = field.section();
            if current != Some(section) {
                rows.push(FormRow::Heading(section));
                current = Some(section);
            }
            rows.push(FormRow::Field(index));
        }
        rows
    }

    fn draw_preview(&mut self, frame: &mut Frame, area: Rect) {
        let title = match self.export_size {
            Some((w, h)) => format!(" Preview  {w}x{h} "),
            None => " Preview ".to_string(),
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(self.palette.border())
            .title(Span::styled(title, self.palette.title()))
            .style(Style::default().bg(to_color(self.palette.background)));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.is_empty() {
            frame.render_widget(
                Paragraph::new("no snippet yet \u{2014} press i to open $EDITOR")
                    .style(self.palette.label())
                    .wrap(Wrap { trim: true }),
                inner,
            );
            return;
        }

        if let Some(error) = &self.error {
            frame.render_widget(
                Paragraph::new(error.as_str())
                    .style(self.palette.fg(self.palette.accent))
                    .wrap(Wrap { trim: true }),
                inner,
            );
            return;
        }

        let Some(protocol) = &mut self.preview else {
            frame.render_widget(
                Paragraph::new("rendering…")
                    .style(self.palette.label())
                    .alignment(Alignment::Center),
                inner,
            );
            return;
        };

        let resize = Resize::Fit(None);
        let fitted = protocol.size_for(resize.clone(), inner);
        let centered = Rect {
            x: inner.x + inner.width.saturating_sub(fitted.width) / 2,
            y: inner.y + inner.height.saturating_sub(fitted.height) / 2,
            width: fitted.width.min(inner.width),
            height: fitted.height.min(inner.height),
        };
        frame.render_stateful_widget(StatefulImage::new().resize(resize), centered, protocol);
    }

    fn draw_picker(&mut self, frame: &mut Frame) {
        let Some(picker) = &self.picker else {
            return;
        };
        let area = centered_rect(48, 20, frame.area());
        frame.render_widget(Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(self.palette.fg(self.palette.accent))
            .title(Span::styled(
                format!(" {} ", picker.title),
                self.palette.title(),
            ))
            .style(Style::default().bg(to_color(self.palette.background)));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(inner);

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("search ", self.palette.label()),
                Span::styled(picker.query(), self.palette.base()),
                Span::styled("\u{2588}", self.palette.fg(self.palette.accent)),
            ])),
            rows[0],
        );

        let height = rows[1].height as usize;
        let (start, end) = picker.window(height);
        let entries: Vec<&str> = picker.matches().collect();
        let lines: Vec<Line> = if entries.is_empty() {
            vec![Line::from(Span::styled(
                "  no matches",
                self.palette.label(),
            ))]
        } else {
            entries[start..end]
                .iter()
                .enumerate()
                .map(|(offset, name)| {
                    let selected = start + offset == picker.selected_index();
                    let text = format!(" {name:<width$}", width = rows[1].width as usize);
                    Span::styled(
                        text,
                        if selected {
                            self.palette.selection()
                        } else {
                            self.palette.base()
                        },
                    )
                    .into()
                })
                .collect()
        };
        frame.render_widget(Paragraph::new(lines), rows[1]);

        frame.render_widget(
            Paragraph::new(Span::styled(
                format!(
                    " {} of {}  ↑↓ choose  enter select  esc cancel",
                    picker.match_count(),
                    picker.total_count()
                ),
                self.palette.label(),
            )),
            rows[2],
        );
    }

    fn draw_status(&self, frame: &mut Frame, area: Rect) {
        let line = match &self.status {
            Some((message, _)) => Line::from(vec![
                Span::styled(" ", self.palette.bar()),
                Span::styled(
                    message.clone(),
                    self.palette
                        .bar()
                        .fg(to_color(self.palette.accent))
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            None => {
                let field = FIELDS[self.selected];
                let primary = if field.is_picker() {
                    "enter search"
                } else if field.is_toggle() {
                    "space toggle"
                } else {
                    "h/l adjust"
                };
                let mut spans = vec![Span::styled(" ", self.palette.bar())];
                for (key, what) in [
                    ("j/k", "move"),
                    (
                        primary.split(' ').next().unwrap(),
                        primary.split(' ').nth(1).unwrap(),
                    ),
                    ("e", "export"),
                    ("c", "copy"),
                    ("i", "edit"),
                    ("s", "save"),
                    ("p", "print cmd"),
                    ("r", "reload"),
                    ("?", "help"),
                    ("q", "quit"),
                ] {
                    spans.push(Span::styled(
                        key,
                        self.palette
                            .bar()
                            .fg(to_color(self.palette.accent))
                            .add_modifier(Modifier::BOLD),
                    ));
                    spans.push(Span::styled(format!(" {what}   "), self.palette.bar()));
                }
                Line::from(spans)
            }
        };
        frame.render_widget(Paragraph::new(line).style(self.palette.bar()), area);
    }
}

fn draw_help(frame: &mut Frame, palette: &Palette) {
    let area = centered_rect(58, 20, frame.area());
    frame.render_widget(Clear, area);

    let key = |k: &'static str, what: &'static str| {
        Line::from(vec![
            Span::styled(
                format!("  {k:<16}"),
                palette.fg(palette.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(what, palette.base()),
        ])
    };
    let heading = |text: &'static str| {
        Line::from(Span::styled(
            text,
            palette.label().add_modifier(Modifier::BOLD),
        ))
    };

    let text = vec![
        heading(" NAVIGATE"),
        key("j / k, ↓ / ↑", "move between settings"),
        key("h / l, ← / →", "adjust the selected setting"),
        key("space", "toggle a boolean setting"),
        key("enter, /", "search a long list (theme, language)"),
        key("g / G", "jump to first / last setting"),
        Line::from(""),
        heading(" DO"),
        key("e", "export a PNG"),
        key("c", "copy the image to the clipboard"),
        key("i", "edit the snippet in $EDITOR"),
        key("s", "save these settings as the default"),
        key("p", "quit and print the equivalent command"),
        key("r", "reload the source file"),
        Line::from(""),
        key("q, esc", "quit"),
        Line::from(""),
        Line::from(Span::styled("  press any key to close", palette.label())),
    ];

    frame.render_widget(
        Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(palette.fg(palette.accent))
                    .title(Span::styled(" Help ", palette.title())),
            )
            .style(Style::default().bg(to_color(palette.background))),
        area,
    );
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum FormRow {
    Heading(Section),
    Field(usize),
}

fn build_palette(renderer: &Renderer, config: &RenderConfig) -> Palette {
    let themes = renderer.themes();
    let chrome = themes
        .chrome(&config.theme)
        .unwrap_or_else(|_| themes.chrome("warm").expect("the warm theme is built in"));
    let syntax = themes
        .syntax(&config.syntax_theme)
        .or_else(|_| themes.syntax(&chrome.syntax_theme))
        .or_else(|_| themes.syntax("warm"))
        .expect("the warm syntax theme is built in");
    Palette::new(chrome, &snapcode_core::theme::accents_of(syntax))
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

fn shell_quote(value: &str) -> String {
    let safe = !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-/=:+,@".contains(c));
    if safe {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_only_what_needs_it() {
        assert_eq!(shell_quote("Feed.swift"), "Feed.swift");
        assert_eq!(shell_quote("src/a-b_c.rs"), "src/a-b_c.rs");
        assert_eq!(shell_quote("my file.swift"), "'my file.swift'");
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("Solarized (dark)"), "'Solarized (dark)'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn centered_rect_fits_inside_a_small_area() {
        let area = Rect::new(0, 0, 10, 5);
        let centered = centered_rect(56, 17, area);
        assert!(centered.width <= area.width);
        assert!(centered.height <= area.height);
        assert!(centered.right() <= area.right());
        assert!(centered.bottom() <= area.bottom());
    }
}
