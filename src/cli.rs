use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TerminalPreviewMode {
    /// Use Chafa's terminal detection, including symbol fallback.
    Auto,
    /// Only attempt a terminal pixel-graphics protocol.
    Graphics,
    /// Always use Unicode symbol rendering in an interactive terminal.
    Symbols,
    /// Disable terminal previews.
    Never,
}

#[derive(Debug, Parser)]
#[command(name = "gfty-label", version, about)]
pub struct Cli {
    /// Also make fonts installed on the host available to the renderer.
    #[arg(long, global = true)]
    pub system_fonts: bool,

    /// Inline terminal preview mode.
    #[arg(long, global = true, value_enum, default_value_t = TerminalPreviewMode::Auto)]
    pub terminal_preview: TerminalPreviewMode,

    /// Maximum terminal preview width in character cells.
    #[arg(long, global = true, default_value_t = 60)]
    pub terminal_preview_width: u16,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Validate a label and all of its references.
    Validate { label: PathBuf },

    /// Render a label as a preview SVG.
    Render {
        label: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Export a label as compact Onshape JSON.
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

        /// Write JSON to PATH, or to stdout when PATH is omitted.
        #[arg(long, num_args = 0..=1, default_missing_value = "-", value_name = "PATH")]
        json: Option<PathBuf>,
    },

    /// List template paths below the current project root.
    ListTemplates,

    /// List icon paths below the current project root.
    ListIcons,

    /// List label paths below the current project root.
    ListLabels,

    /// List templates, icons, and labels below the current project root.
    List,

    /// Arrange labels into a dimension-constrained plate grid.
    Plate {
        /// Maximum plate width and height, for example: --dimensions 200mm 250mm.
        #[arg(long, value_names = ["WIDTH", "HEIGHT"], num_args = 2, required = true)]
        dimensions: Vec<String>,

        #[arg(long, default_value = "5mm")]
        column_gap: String,

        #[arg(long, default_value = "5mm")]
        row_gap: String,

        #[arg(long)]
        svg: Option<PathBuf>,

        #[arg(long)]
        json: Option<PathBuf>,

        /// Labels in top-left, row-major order. Repeat a path to repeat a label.
        #[arg(required = true)]
        labels: Vec<PathBuf>,
    },

    /// Rebuild a label whenever project inputs change.
    Watch {
        label: PathBuf,

        #[arg(long)]
        svg: Option<PathBuf>,

        #[arg(long)]
        json: Option<PathBuf>,
    },
}
