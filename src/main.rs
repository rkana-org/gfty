mod cli;
mod color;
mod compose;
mod config;
mod export;
mod layout;
mod plate;
mod quick;
mod svg;
mod template;
mod text;
mod watch;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let system_fonts = cli.system_fonts;
    match cli.command {
        Command::Validate { label } => {
            let loaded = config::LoadedLabel::load(&label)?;
            loaded.validate()?;
            println!("{}", loaded.path.display());
        }
        Command::Render { label, output } => {
            let loaded = config::LoadedLabel::load(&label)?;
            let svg = compose::render_label_svg(&loaded, system_fonts)?;
            std::fs::write(&output, svg)?;
            println!("{}", output.display());
        }
        Command::Export { label, output } => {
            let loaded = config::LoadedLabel::load(&label)?;
            let rendered = compose::render_label(&loaded, system_fonts)?;
            let document = export::export_rendered(&rendered)?;
            write_json(&output, &document)?;
            println!("{}", output.display());
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
            let loaded = quick::build_quick_label(&template, &text, &icon)?;
            let rendered = compose::render_label(&loaded, system_fonts)?;
            if let Some(output) = svg {
                std::fs::write(&output, &rendered.svg)?;
                println!("{}", output.display());
            }
            if let Some(output) = json {
                write_json(&output, &export::export_rendered(&rendered)?)?;
                println!("{}", output.display());
            }
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
            let output = plate::build_plate(&labels, columns, &column_gap, &row_gap, system_fonts)?;
            if let Some(path) = svg {
                std::fs::write(&path, output.svg)?;
                println!("{}", path.display());
            }
            if let Some(path) = json {
                write_json(&path, &output.document)?;
                println!("{}", path.display());
            }
        }
        Command::Watch { label, svg, json } => {
            watch::watch_label(&label, svg.as_deref(), json.as_deref(), system_fonts)?;
        }
    }
    Ok(())
}

fn write_json(path: &std::path::Path, document: &export::ExportDocument) -> Result<()> {
    let mut json = serde_json::to_vec(document)?;
    json.push(b'\n');
    std::fs::write(path, json)?;
    Ok(())
}
