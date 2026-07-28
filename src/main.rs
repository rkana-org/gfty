mod cli;
mod color;
mod compose;
mod config;
mod export;
mod layout;
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
            let mut json = serde_json::to_vec(&document)?;
            json.push(b'\n');
            std::fs::write(&output, json)?;
            println!("{}", output.display());
        }
        Command::Quick { .. } => anyhow::bail!("quick is not implemented yet"),
        Command::Watch { .. } => anyhow::bail!("watch is not implemented yet"),
    }
    Ok(())
}
