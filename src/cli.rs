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
#[command(name = "gfty", version, about)]
pub struct Cli {
    /// Also make fonts installed on the host available to the renderer.
    #[arg(long, global = true)]
    pub system_fonts: bool,

    /// Add a directory of fonts. May be repeated.
    #[arg(long, global = true, value_name = "PATH")]
    pub font_dir: Vec<PathBuf>,

    /// Root used by pathless label validate; defaults to the current directory.
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
    /// Dispatch a versioned TOML file to its configured remote STEP exporter.
    Export(GenericExportArgs),

    /// Author, inspect, render, and export labels.
    Label(LabelArgs),

    /// Validate, inspect, and export Gridfinity bins.
    Bin(BinArgs),

    /// Export the configuration-free standard base connector pin.
    ConnectorPin(ConnectorPinArgs),
}

#[derive(Debug, Args)]
pub struct LabelArgs {
    #[command(subcommand)]
    pub command: LabelCommand,
}

#[derive(Debug, Subcommand)]
pub enum LabelCommand {
    /// Validate one label, or every label below labels/ when omitted.
    Validate { label: Option<PathBuf> },

    /// Render a label as a preview SVG.
    Render {
        label: PathBuf,
        /// Write SVG to PATH; when omitted, render only in the terminal.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Create an unsaved label from command-line values.
    Create(CreateArgs),

    /// Inspect a label TOML, template SVG, or icon SVG.
    Inspect { file: PathBuf },

    /// Rebuild a label whenever its inputs change.
    Watch {
        label: PathBuf,

        #[arg(long)]
        svg: Option<PathBuf>,
    },

    /// Generate a configured label in Onshape and download a grouped STEP.
    Export(ExportArgs),

    /// Arrange and export multi-label plates.
    Plate(PlateArgs),
}

#[derive(Debug, Args)]
pub struct PlateArgs {
    #[command(subcommand)]
    pub command: PlateCommand,
}

#[derive(Debug, Subcommand)]
pub enum PlateCommand {
    /// Create a dimension-constrained plate preview.
    Create(PlateCreateArgs),

    /// Generate a configured plate in Onshape and download a grouped STEP.
    Export(ExportPlateArgs),
}

#[derive(Debug, Args)]
pub struct CreateArgs {
    #[arg(long)]
    pub template: PathBuf,

    /// Filament used for the blank prototype body.
    #[arg(long, default_value_t = 0)]
    pub filament: u32,

    /// Repeat as: --text ID CONTENT
    #[arg(long, value_names = ["ID", "CONTENT"], num_args = 2, action = clap::ArgAction::Append)]
    pub text: Vec<String>,

    /// Repeat as: --icon BOX ICON
    #[arg(long, value_names = ["BOX", "ICON"], num_args = 2, action = clap::ArgAction::Append)]
    pub icon: Vec<String>,

    /// Write the rendered SVG to PATH.
    #[arg(long)]
    pub svg: Option<PathBuf>,

    /// Save this invocation as a reusable label TOML file.
    #[arg(long, value_name = "PATH")]
    pub save: Option<PathBuf>,

    /// Export directly to STEP, defaulting to label.step when PATH is omitted.
    #[arg(long, num_args = 0..=1, default_missing_value = "label.step", value_name = "PATH")]
    pub export: Option<PathBuf>,

    /// Legacy Gridfinity Ultimate JSON required by --export unless --bin is used.
    #[arg(long, value_name = "PATH", conflicts_with = "bin")]
    pub gridfinity_config: Option<PathBuf>,

    /// Versioned bin TOML used by --export and saved into the label definition.
    #[arg(long, value_name = "PATH", conflicts_with = "gridfinity_config")]
    pub bin: Option<PathBuf>,

    /// Protected TOML file containing access-key and secret-key.
    #[arg(long, value_name = "PATH")]
    pub onshape_credentials: Option<PathBuf>,

    /// Immutable Onshape label model version URL.
    #[arg(long, default_value = DEFAULT_LABEL_MODEL_URL, value_name = "URL")]
    pub onshape_model: String,

    /// Replace an existing STEP output.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct PlateCreateArgs {
    /// Maximum plate width and height, for example: --dimensions 200mm 250mm.
    #[arg(long, value_names = ["WIDTH", "HEIGHT"], num_args = 2, required = true)]
    pub dimensions: Vec<String>,

    #[arg(long, default_value = "5mm")]
    pub column_gap: String,

    #[arg(long, default_value = "5mm")]
    pub row_gap: String,

    /// Write the rendered SVG to PATH; otherwise preview in the terminal.
    #[arg(long)]
    pub svg: Option<PathBuf>,

    /// Labels in top-left, row-major order. Repeat a path to repeat a label.
    #[arg(required = true)]
    pub labels: Vec<PathBuf>,
}

#[derive(Debug, Args)]
pub struct GenericExportArgs {
    /// Versioned label or bin TOML to export.
    pub file: PathBuf,

    /// Legacy Gridfinity Ultimate JSON used by label exports.
    #[arg(long, value_name = "PATH", conflicts_with = "bin")]
    pub gridfinity_config: Option<PathBuf>,

    /// Override the bin TOML referenced by a label.
    #[arg(long, value_name = "PATH", conflicts_with = "gridfinity_config")]
    pub bin: Option<PathBuf>,

    /// Select an exact named part from a bin configuration.
    #[arg(long, value_enum)]
    pub component: Option<crate::bin_config::BinComponent>,

    /// Destination STEP path; defaults to the input name in the current directory.
    #[arg(short, long, value_name = "PATH")]
    pub output: Option<PathBuf>,

    /// Download a 512×512 isometric PNG preview (Gridfinity exports only).
    #[arg(long, value_name = "PATH")]
    pub image: Option<PathBuf>,

    /// Protected TOML file containing access-key and secret-key.
    #[arg(long, value_name = "PATH")]
    pub onshape_credentials: Option<PathBuf>,

    /// Override the immutable Onshape model version URL for this file kind.
    #[arg(long, value_name = "URL")]
    pub onshape_model: Option<String>,

    /// Bypass the normalized runtime artifact cache.
    #[arg(long)]
    pub no_cache: bool,

    /// Replace an existing output file.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Label TOML to render and export.
    pub label: PathBuf,

    #[command(flatten)]
    pub remote: RemoteExportArgs,
}

#[derive(Debug, Args)]
pub struct ExportPlateArgs {
    /// Maximum plate width and height, for example: --dimensions 200mm 250mm.
    #[arg(long, value_names = ["WIDTH", "HEIGHT"], num_args = 2, required = true)]
    pub dimensions: Vec<String>,

    #[arg(long, default_value = "5mm")]
    pub column_gap: String,

    #[arg(long, default_value = "5mm")]
    pub row_gap: String,

    /// Labels in top-left, row-major order. Repeat a path to repeat a label.
    #[arg(required = true)]
    pub labels: Vec<PathBuf>,

    #[command(flatten)]
    pub remote: RemoteExportArgs,
}

#[derive(Debug, Args)]
pub struct RemoteExportArgs {
    /// Legacy Gridfinity Ultimate JSON for the label prototype.
    #[arg(long, value_name = "PATH", conflicts_with = "bin")]
    pub gridfinity_config: Option<PathBuf>,

    /// Bin TOML for the label prototype; defaults to the label's `bin` field.
    #[arg(long, value_name = "PATH", conflicts_with = "gridfinity_config")]
    pub bin: Option<PathBuf>,

    /// Destination STEP path; defaults to the input name in the current directory.
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

#[derive(Debug, Args)]
pub struct ConnectorPinArgs {
    #[command(subcommand)]
    pub command: ConnectorPinCommand,
}

#[derive(Debug, Subcommand)]
pub enum ConnectorPinCommand {
    /// Export the standard connector pin as an isolated STEP and optional PNG.
    Export(StandaloneGridfinityExportArgs),
}

#[derive(Debug, Args)]
pub struct StandaloneGridfinityExportArgs {
    /// Destination STEP path; defaults to connector-pin.step.
    #[arg(short, long, value_name = "PATH")]
    pub output: Option<PathBuf>,

    /// Download a 512×512 isometric PNG preview.
    #[arg(long, value_name = "PATH")]
    pub image: Option<PathBuf>,

    /// Protected TOML file containing access-key and secret-key.
    #[arg(long, value_name = "PATH")]
    pub onshape_credentials: Option<PathBuf>,

    /// Immutable Gridfinity Ultimate model version URL.
    #[arg(long, default_value = crate::bin_config::DEFAULT_BIN_MODEL_URL, value_name = "URL")]
    pub onshape_model: String,

    /// Bypass the normalized runtime artifact cache.
    #[arg(long)]
    pub no_cache: bool,

    /// Replace existing output files.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct BinArgs {
    #[command(subcommand)]
    pub command: BinCommand,
}

#[derive(Debug, Subcommand)]
pub enum BinCommand {
    /// Validate a versioned bin TOML file.
    Validate { bin: PathBuf },

    /// Inspect resolved dimensions, dividers, supports, and generated parts.
    Inspect { bin: PathBuf },

    /// Generate a configured bin in Onshape and download a grouped STEP.
    Export(BinExportArgs),
}

#[derive(Debug, Args)]
pub struct BinExportArgs {
    /// Versioned bin TOML to export.
    pub bin: PathBuf,

    /// Export the complete configuration or one exact named part. Defaults to
    /// `all` for legacy version-1 bins and `bin` for version-2 bin bodies.
    #[arg(long, value_enum)]
    pub component: Option<crate::bin_config::BinComponent>,

    /// Destination STEP path; defaults to the input name in the current directory.
    #[arg(short, long, value_name = "PATH")]
    pub output: Option<PathBuf>,

    /// Download a 512×512 isometric PNG preview.
    #[arg(long, value_name = "PATH")]
    pub image: Option<PathBuf>,

    /// Protected TOML file containing access-key and secret-key.
    #[arg(long, value_name = "PATH")]
    pub onshape_credentials: Option<PathBuf>,

    /// Immutable Gridfinity Ultimate model version URL.
    #[arg(long, default_value = crate::bin_config::DEFAULT_BIN_MODEL_URL, value_name = "URL")]
    pub onshape_model: String,

    /// Bypass the normalized runtime artifact cache.
    #[arg(long)]
    pub no_cache: bool,

    /// Replace an existing output file.
    #[arg(long)]
    pub force: bool,
}
