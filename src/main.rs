mod cli;
mod color;
mod compose;
mod config;
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
        Command::Export { .. } => anyhow::bail!("export is not implemented yet"),
        Command::Quick { .. } => anyhow::bail!("quick is not implemented yet"),
        Command::Watch { .. } => anyhow::bail!("watch is not implemented yet"),
    }
    Ok(())
}
