use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "gfty-label", version, about)]
pub struct Cli {
    /// Also make fonts installed on the host available to the renderer.
    #[arg(long, global = true)]
    pub system_fonts: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Validate a saved label and all of its references.
    Validate { label: PathBuf },

    /// Render a saved label as a preview SVG.
    Render {
        label: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Export a saved label as compact Onshape JSON.
    Export {
        label: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Build an unsaved label from command-line values.
    Quick {
        #[arg(long)]
        template: PathBuf,

        /// Repeat as: --text ID CONTENT
        #[arg(long, value_names = ["ID", "CONTENT"], num_args = 2, action = clap::ArgAction::Append)]
        text: Vec<String>,

        /// Repeat as: --icon BOX ICON
        #[arg(long, value_names = ["BOX", "ICON"], num_args = 2, action = clap::ArgAction::Append)]
        icon: Vec<String>,

        #[arg(long)]
        svg: Option<PathBuf>,

        #[arg(long)]
        json: Option<PathBuf>,
    },

    /// Arrange saved labels into a fixed-column plate grid.
    Plate {
        /// Number of columns in the fixed-width grid.
        #[arg(long)]
        columns: usize,

        #[arg(long, default_value = "0mm")]
        column_gap: String,

        #[arg(long, default_value = "0mm")]
        row_gap: String,

        #[arg(long)]
        svg: Option<PathBuf>,

        #[arg(long)]
        json: Option<PathBuf>,

        /// Labels in top-left, row-major order. Repeat a path to repeat a label.
        #[arg(required = true)]
        labels: Vec<PathBuf>,
    },

    /// Rebuild a saved label whenever project inputs change.
    Watch {
        label: PathBuf,

        #[arg(long)]
        svg: Option<PathBuf>,

        #[arg(long)]
        json: Option<PathBuf>,
    },
}
