use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

pub const DEFAULT_LABEL_MODEL_URL: &str = "https://cad.onshape.com/documents/089ad0a2edf08cd2cfdc9875/v/02d1ce92af09ce405aff8f7d/e/5bba513a46b691f2bf439aaa";

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TerminalPreviewMode {
    /// Detect Kitty, iTerm2, or Sixel support, with a symbol fallback.
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

    /// Add a directory of fonts. May be repeated.
    #[arg(long, global = true, value_name = "PATH")]
    pub font_dir: Vec<PathBuf>,

    /// Root used by pathless validate; defaults to the current directory.
    #[arg(long, global = true, value_name = "PATH")]
    pub root: Option<PathBuf>,

    /// Inline terminal preview mode.
    #[arg(long, global = true, value_enum, default_value_t = TerminalPreviewMode::Auto)]
    pub terminal_preview: TerminalPreviewMode,

    /// Maximum terminal preview width in character cells.
    #[arg(long, global = true, default_value_t = 60)]
    pub terminal_preview_width: u16,

    /// Show a terminal preview while inspecting a file.
    #[arg(long, global = true)]
    pub preview: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Validate one label, or every label below labels/ when omitted.
    Validate { label: Option<PathBuf> },

    /// Render a label as a preview SVG.
    Render {
        label: PathBuf,
        /// Write SVG to PATH; when omitted, render only in the terminal.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Build label.svg and label.json in an output directory.
    Build {
        label: PathBuf,
        #[arg(short, long, value_name = "DIR")]
        output: PathBuf,
    },

    /// Generate a configured label in Onshape and download a grouped STEP.
    Export(ExportArgs),

    /// Build an unsaved label from command-line values.
    Quick {
        #[arg(long)]
        template: PathBuf,

        /// Filament used for the blank prototype body.
        #[arg(long, default_value_t = 0)]
        filament: u32,

        /// Repeat as: --text ID CONTENT
        #[arg(long, value_names = ["ID", "CONTENT"], num_args = 2, action = clap::ArgAction::Append)]
        text: Vec<String>,

        /// Repeat as: --icon BOX ICON
        #[arg(long, value_names = ["BOX", "ICON"], num_args = 2, action = clap::ArgAction::Append)]
        icon: Vec<String>,

        #[arg(long)]
        svg: Option<PathBuf>,

        /// Save this invocation as a reusable label TOML file.
        #[arg(long, value_name = "PATH")]
        save: Option<PathBuf>,

        /// Write JSON to PATH, or to stdout when PATH is omitted.
        #[arg(long, num_args = 0..=1, default_missing_value = "-", value_name = "PATH")]
        json: Option<PathBuf>,
    },

    /// Inspect a label TOML, template SVG, or icon SVG.
    Inspect { file: PathBuf },

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

        /// Write JSON to PATH, or to stdout when PATH is omitted.
        #[arg(long, num_args = 0..=1, default_missing_value = "-", value_name = "PATH")]
        json: Option<PathBuf>,

        /// Labels in top-left, row-major order. Repeat a path to repeat a label.
        #[arg(required = true)]
        labels: Vec<PathBuf>,
    },

    /// Rebuild a label whenever its inputs change.
    Watch {
        label: PathBuf,

        #[arg(long)]
        svg: Option<PathBuf>,

        #[arg(long)]
        json: Option<PathBuf>,
    },
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Label TOML to render and export.
    pub label: PathBuf,

    /// Gridfinity Ultimate configuration JSON for the label prototype.
    #[arg(long, value_name = "PATH")]
    pub gridfinity_config: PathBuf,

    /// Destination STEP path; defaults to LABEL's file stem in the current directory.
    #[arg(short, long, value_name = "PATH")]
    pub output: Option<PathBuf>,

    /// Protected TOML file containing access-key and secret-key.
    #[arg(long, value_name = "PATH")]
    pub onshape_credentials: Option<PathBuf>,

    /// Immutable Onshape label model version URL.
    #[arg(long, default_value = DEFAULT_LABEL_MODEL_URL, value_name = "URL")]
    pub onshape_model: String,

    /// Replace an existing output file.
    #[arg(long)]
    pub force: bool,
}
