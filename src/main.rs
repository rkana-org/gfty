mod cli;
mod color;
mod compose;
mod config;
mod export;
mod layout;
mod list;
mod plate;
mod quick;
mod svg;
mod template;
mod terminal_preview;
mod text;
mod watch;

use std::io::{IsTerminal, Write};

use anyhow::{Context, Result};
use clap::Parser;

use crate::cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let system_fonts = cli.system_fonts;
    let preview_options = terminal_preview::PreviewOptions {
        mode: cli.terminal_preview,
        width: cli.terminal_preview_width,
    };
    match cli.command {
        Command::Validate { label } => {
            let loaded = config::LoadedLabel::load(&label)
                .with_context(|| format!("failed to load label {}", label.display()))?;
            loaded
                .validate()
                .with_context(|| format!("failed to validate label {}", loaded.path.display()))?;
            println!("Validated label: {}", loaded.path.display());
        }
        Command::Render { label, output } => {
            let loaded = config::LoadedLabel::load(&label)
                .with_context(|| format!("failed to load label {}", label.display()))?;
            let svg = compose::render_label_svg(&loaded, system_fonts)
                .with_context(|| format!("failed to render label {}", loaded.path.display()))?;
            std::fs::write(&output, &svg)
                .with_context(|| format!("failed to write SVG {}", output.display()))?;
            println!("Rendered SVG: {}", output.display());
            try_preview(
                &svg,
                &loaded.path.display().to_string(),
                preview_options,
                false,
            );
        }
        Command::Export { label, output } => {
            let loaded = config::LoadedLabel::load(&label)
                .with_context(|| format!("failed to load label {}", label.display()))?;
            let rendered = compose::render_label(&loaded, system_fonts)
                .with_context(|| format!("failed to render label {}", loaded.path.display()))?;
            let document = export::export_rendered(&rendered)
                .with_context(|| format!("failed to export label {}", loaded.path.display()))?;
            let stdout = write_json(&output, &document)?;
            print_success(stdout, format_args!("Exported JSON: {}", output.display()));
        }
        Command::Quick {
            template,
            text,
            icon,
            svg,
            json,
        } => {
            if svg.is_none() && json.is_none() {
                anyhow::bail!("quick needs at least one of --svg or --json");
            }
            let loaded = quick::build_quick_label(&template, &text, &icon)
                .context("failed to build quick label configuration")?;
            let rendered = compose::render_label(&loaded, system_fonts)
                .context("failed to render quick label")?;
            try_preview(&rendered.svg, "quick label", preview_options, false);
            let json_to_stdout = json.as_deref().is_some_and(is_stdout_path);
            if let Some(output) = svg {
                std::fs::write(&output, &rendered.svg)
                    .with_context(|| format!("failed to write SVG {}", output.display()))?;
                print_success(
                    json_to_stdout,
                    format_args!("Rendered SVG: {}", output.display()),
                );
            }
            if let Some(output) = json {
                let document =
                    export::export_rendered(&rendered).context("failed to export quick label")?;
                let stdout = write_json(&output, &document)?;
                print_success(stdout, format_args!("Exported JSON: {}", output.display()));
            }
        }
        Command::ListTemplates => {
            let entries = list::discover()?;
            print_values(&entries.templates);
            preview_entries(
                &entries,
                &entries.templates,
                EntryKind::Svg,
                system_fonts,
                preview_options,
            );
        }
        Command::ListIcons => {
            let entries = list::discover()?;
            print_values(&entries.icons);
            preview_entries(
                &entries,
                &entries.icons,
                EntryKind::Svg,
                system_fonts,
                preview_options,
            );
        }
        Command::ListLabels => {
            let entries = list::discover()?;
            print_values(&entries.labels);
            preview_entries(
                &entries,
                &entries.labels,
                EntryKind::Label,
                system_fonts,
                preview_options,
            );
        }
        Command::List => {
            let entries = list::discover()?;
            list::print_group("Templates", &entries.templates);
            preview_entries(
                &entries,
                &entries.templates,
                EntryKind::Svg,
                system_fonts,
                preview_options,
            );
            list::print_group("Icons", &entries.icons);
            preview_entries(
                &entries,
                &entries.icons,
                EntryKind::Svg,
                system_fonts,
                preview_options,
            );
            list::print_group("Labels", &entries.labels);
            preview_entries(
                &entries,
                &entries.labels,
                EntryKind::Label,
                system_fonts,
                preview_options,
            );
        }
        Command::Plate {
            dimensions,
            column_gap,
            row_gap,
            svg,
            json,
            labels,
        } => {
            if svg.is_none() && json.is_none() {
                anyhow::bail!("plate needs at least one of --svg or --json");
            }
            let output =
                plate::build_plate(&labels, &dimensions, &column_gap, &row_gap, system_fonts)
                    .context("failed to generate plate")?;
            try_preview(&output.svg, "plate", preview_options, false);
            if let Some(path) = svg {
                std::fs::write(&path, output.svg)
                    .with_context(|| format!("failed to write plate SVG {}", path.display()))?;
                println!("Generated plate SVG: {}", path.display());
            }
            if let Some(path) = json {
                let stdout = write_json(&path, &output.document)?;
                print_success(
                    stdout,
                    format_args!("Generated plate JSON: {}", path.display()),
                );
            }
        }
        Command::Watch { label, svg, json } => {
            watch::watch_label(
                &label,
                svg.as_deref(),
                json.as_deref(),
                system_fonts,
                preview_options,
            )
            .with_context(|| format!("failed to watch label {}", label.display()))?;
        }
    }
    Ok(())
}

fn write_json(path: &std::path::Path, document: &export::ExportDocument) -> Result<bool> {
    let mut json = serde_json::to_vec(document).context("failed to serialize Onshape JSON")?;
    json.push(b'\n');
    if is_stdout_path(path) {
        std::io::stdout()
            .lock()
            .write_all(&json)
            .context("failed to write JSON to stdout")?;
        Ok(true)
    } else {
        std::fs::write(path, json)
            .with_context(|| format!("failed to write JSON {}", path.display()))?;
        Ok(false)
    }
}

fn is_stdout_path(path: &std::path::Path) -> bool {
    path == std::path::Path::new("-")
}

#[derive(Clone, Copy)]
enum EntryKind {
    Svg,
    Label,
}

fn print_values(values: &[String]) {
    for value in values {
        println!("{value}");
    }
}

fn preview_entries(
    entries: &list::ProjectEntries,
    values: &[String],
    kind: EntryKind,
    system_fonts: bool,
    options: terminal_preview::PreviewOptions,
) {
    if !options.enabled() || !std::io::stdout().is_terminal() {
        return;
    }
    for value in values {
        let path = entries.root.join(value);
        let result = match kind {
            EntryKind::Svg => std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read SVG {}", path.display()))
                .and_then(|source| {
                    svg::normalize_svg(
                        &source,
                        path.parent().expect("listed SVG has a parent"),
                        &entries.root,
                        system_fonts,
                    )
                    .with_context(|| format!("failed to render listed SVG {}", path.display()))
                }),
            EntryKind::Label => config::LoadedLabel::load(&path)
                .with_context(|| format!("failed to load listed label {}", path.display()))
                .and_then(|label| {
                    compose::render_label_svg(&label, system_fonts).with_context(|| {
                        format!("failed to render listed label {}", path.display())
                    })
                }),
        };
        match result {
            Ok(svg) => try_preview(&svg, value, options, false),
            Err(error) => eprintln!("Preview failed for {value}: {error:#}"),
        }
    }
}

fn try_preview(svg: &str, label: &str, options: terminal_preview::PreviewOptions, clear: bool) {
    if let Err(error) = terminal_preview::show_svg(svg, label, options, clear) {
        eprintln!("Terminal preview failed: {error:#}");
    }
}

fn print_success(use_stderr: bool, message: std::fmt::Arguments<'_>) {
    if use_stderr {
        eprintln!("{message}");
    } else {
        println!("{message}");
    }
}
