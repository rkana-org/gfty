mod cli;
mod color;
mod compose;
mod config;
mod export;
mod layout;
mod quick;
mod svg;
mod template;
mod text;

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
        Command::Watch { .. } => anyhow::bail!("watch is not implemented yet"),
    }
    Ok(())
}

fn write_json(path: &std::path::Path, document: &export::ExportDocument) -> Result<()> {
    let mut json = serde_json::to_vec(document)?;
    json.push(b'\n');
    std::fs::write(path, json)?;
    Ok(())
}
