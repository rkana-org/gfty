mod cli;
mod color;
mod config;
mod layout;
mod template;
mod text;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Validate { label } => {
            let loaded = config::LoadedLabel::load(&label)?;
            loaded.validate()?;
            println!("{}", label.display());
        }
        Command::Render { .. } => anyhow::bail!("render is not implemented yet"),
        Command::Export { .. } => anyhow::bail!("export is not implemented yet"),
        Command::Quick { .. } => anyhow::bail!("quick is not implemented yet"),
        Command::Watch { .. } => anyhow::bail!("watch is not implemented yet"),
    }
    Ok(())
}
