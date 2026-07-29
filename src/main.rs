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
mod text;
mod watch;

use std::io::Write;

use anyhow::{Context, Result};
use clap::Parser;

use crate::cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let system_fonts = cli.system_fonts;
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
            std::fs::write(&output, svg)
                .with_context(|| format!("failed to write SVG {}", output.display()))?;
            println!("Rendered SVG: {}", output.display());
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
            for value in list::discover()?.templates {
                println!("{value}");
            }
        }
        Command::ListIcons => {
            for value in list::discover()?.icons {
                println!("{value}");
            }
        }
        Command::ListLabels => {
            for value in list::discover()?.labels {
                println!("{value}");
            }
        }
        Command::List => {
            let entries = list::discover()?;
            list::print_group("Templates", &entries.templates);
            list::print_group("Icons", &entries.icons);
            list::print_group("Labels", &entries.labels);
        }
        Command::Plate {
            columns,
            column_gap,
            row_gap,
            svg,
            json,
            labels,
        } => {
            if svg.is_none() && json.is_none() {
                anyhow::bail!("plate needs at least one of --svg or --json");
            }
            let output = plate::build_plate(&labels, columns, &column_gap, &row_gap, system_fonts)
                .context("failed to generate plate")?;
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
            watch::watch_label(&label, svg.as_deref(), json.as_deref(), system_fonts)
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

fn print_success(use_stderr: bool, message: std::fmt::Arguments<'_>) {
    if use_stderr {
        eprintln!("{message}");
    } else {
        println!("{message}");
    }
}
