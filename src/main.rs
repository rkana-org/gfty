mod bin_config;
mod cli;
mod color;
mod compose;
mod config;
mod create;
mod credentials;
mod export;
mod layout;
mod onshape;
mod plate;
mod step;
mod svg;
mod template;
mod terminal_preview;
mod text;
mod watch;

use anyhow::{Context, Result};
use clap::Parser;
use colored::Colorize;

use crate::cli::{
    BinCommand, BinExportArgs, Cli, Command, CreateArgs, ExportArgs, ExportPlateArgs,
    GenericExportArgs, LabelCommand, PlateCommand, PlateCreateArgs, RemoteExportArgs,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{} {error:#}", "error:".red().bold());
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let font_options = svg::FontOptions {
        system_fonts: cli.system_fonts,
        directories: cli.font_dir,
    };
    let preview_options = terminal_preview::PreviewOptions {
        mode: cli.terminal_preview,
        width: cli.terminal_preview_width,
    };
    match cli.command {
        Command::Export(args) => export_config(args, &font_options),
        Command::Label(label) => run_label_command(
            label.command,
            cli.root.as_deref(),
            cli.preview,
            &font_options,
            preview_options,
        ),
        Command::Bin(bin) => match bin.command {
            BinCommand::Validate { bin } => validate_bin(&bin),
            BinCommand::Inspect { bin } => inspect_bin(&bin),
            BinCommand::Export(args) => export_bin_step(args),
        },
    }
}

fn run_label_command(
    command: LabelCommand,
    root: Option<&std::path::Path>,
    inspect_preview: bool,
    font_options: &svg::FontOptions,
    preview_options: terminal_preview::PreviewOptions,
) -> Result<()> {
    match command {
        LabelCommand::Validate { label } => validate_labels(label.as_deref(), root, font_options),
        LabelCommand::Render { label, output } => {
            let loaded = config::LoadedLabel::load(&label)
                .with_context(|| format!("failed to load label {}", label.display()))?;
            let svg = compose::render_label_svg(&loaded, font_options)
                .with_context(|| format!("failed to render label {}", loaded.path.display()))?;
            if let Some(output) = output {
                std::fs::write(&output, &svg)
                    .with_context(|| format!("failed to write SVG {}", output.display()))?;
                try_preview(
                    &svg,
                    &loaded.path.display().to_string(),
                    preview_options,
                    false,
                );
                Ok(())
            } else {
                show_required_preview(&svg, &loaded.path.display().to_string(), preview_options)
            }
        }
        LabelCommand::Create(args) => create_label(args, font_options, preview_options),
        LabelCommand::Inspect { file } => {
            inspect_file(&file, font_options, preview_options, inspect_preview)
        }
        LabelCommand::Watch { label, svg } => {
            watch::watch_label(&label, svg.as_deref(), font_options, preview_options)
                .with_context(|| format!("failed to watch label {}", label.display()))
        }
        LabelCommand::Export(args) => export_label_step(args, font_options),
        LabelCommand::Plate(plate) => match plate.command {
            PlateCommand::Create(args) => create_plate(args, font_options, preview_options),
            PlateCommand::Export(args) => export_plate_step(args, font_options),
        },
    }
}

fn create_label(
    args: CreateArgs,
    font_options: &svg::FontOptions,
    preview_options: terminal_preview::PreviewOptions,
) -> Result<()> {
    if args.svg.is_none() && args.save.is_none() && args.export.is_none() {
        anyhow::bail!("label create needs at least one of --svg, --save, or --export");
    }
    let mut loaded = create::build_label(&args.template, args.filament, &args.text, &args.icon)
        .context("failed to create label configuration")?;
    if let Some(bin) = &args.bin {
        let bin = bin
            .canonicalize()
            .with_context(|| format!("failed to resolve bin {}", bin.display()))?;
        loaded.config.bin = Some(bin.to_string_lossy().replace('\\', "/"));
    }
    let rendered =
        compose::render_label(&loaded, font_options).context("failed to render label")?;
    try_preview(&rendered.svg, "created label", preview_options, false);
    if let Some(output) = args.save {
        create::save_label(&loaded, &output)?;
    }
    if let Some(output) = args.svg {
        std::fs::write(&output, &rendered.svg)
            .with_context(|| format!("failed to write SVG {}", output.display()))?;
    }
    if let Some(output) = args.export {
        step::ensure_output_available(&output, args.force)?;
        let document = export::export_rendered(&rendered)
            .context("failed to generate geometry for created label")?;
        let remote = RemoteExportArgs {
            gridfinity_config: args.gridfinity_config,
            bin: args.bin,
            output: Some(output.clone()),
            onshape_credentials: args.onshape_credentials,
            onshape_model: args.onshape_model,
            force: args.force,
        };
        let gridfinity_json = resolve_gridfinity_config(&remote, loaded.bin_path().as_deref())?;
        download_document_step(&document, remote, output, "created label", &gridfinity_json)?;
    }
    Ok(())
}

fn create_plate(
    args: PlateCreateArgs,
    font_options: &svg::FontOptions,
    preview_options: terminal_preview::PreviewOptions,
) -> Result<()> {
    let output = plate::build_plate(
        &args.labels,
        &args.dimensions,
        &args.column_gap,
        &args.row_gap,
        font_options,
    )
    .context("failed to generate label plate")?;
    if let Some(path) = args.svg {
        std::fs::write(&path, &output.svg)
            .with_context(|| format!("failed to write plate SVG {}", path.display()))?;
        try_preview(&output.svg, "label plate", preview_options, false);
        Ok(())
    } else {
        show_required_preview(&output.svg, "label plate", preview_options)
    }
}

fn show_required_preview(
    svg: &str,
    name: &str,
    preview_options: terminal_preview::PreviewOptions,
) -> Result<()> {
    let shown = terminal_preview::show_svg(svg, name, preview_options, false)
        .context("failed to render SVG in the terminal")?;
    if !shown {
        anyhow::bail!(
            "terminal preview is unavailable; use an output option or select a supported terminal preview mode"
        );
    }
    Ok(())
}

fn export_config(args: GenericExportArgs, font_options: &svg::FontOptions) -> Result<()> {
    match config::detect_config_kind(&args.file)? {
        config::ConfigKind::Label => {
            if args.component.is_some() {
                anyhow::bail!("--component is only supported for standalone bin exports");
            }
            if args.image.is_some() {
                anyhow::bail!(
                    "--image is currently supported only for bin exports; configured label geometry is too large for Onshape's GET-only shaded-view endpoint"
                );
            }
            export_label_step(
                ExportArgs {
                    label: args.file,
                    remote: RemoteExportArgs {
                        gridfinity_config: args.gridfinity_config,
                        bin: args.bin,
                        output: args.output,
                        onshape_credentials: args.onshape_credentials,
                        onshape_model: args
                            .onshape_model
                            .unwrap_or_else(|| cli::DEFAULT_LABEL_MODEL_URL.to_owned()),
                        force: args.force,
                    },
                },
                font_options,
            )
        }
        config::ConfigKind::Bin => {
            if args.gridfinity_config.is_some() || args.bin.is_some() {
                anyhow::bail!("standalone bin export does not accept --gridfinity-config or --bin");
            }
            export_bin_step(BinExportArgs {
                bin: args.file,
                component: args.component.unwrap_or(bin_config::BinComponent::All),
                output: args.output,
                image: args.image,
                onshape_credentials: args.onshape_credentials,
                onshape_model: args
                    .onshape_model
                    .unwrap_or_else(|| bin_config::DEFAULT_BIN_MODEL_URL.to_owned()),
                force: args.force,
            })
        }
        config::ConfigKind::LabelPlate => {
            anyhow::bail!(
                "saved label-plate TOML is not implemented yet; use `gfty label plate export`"
            )
        }
    }
}

fn export_label_step(args: ExportArgs, font_options: &svg::FontOptions) -> Result<()> {
    let output = args
        .remote
        .output
        .clone()
        .unwrap_or_else(|| default_step_path(&args.label));
    step::ensure_output_available(&output, args.remote.force)?;

    let loaded = config::LoadedLabel::load(&args.label)
        .with_context(|| format!("failed to load label {}", args.label.display()))?;
    let rendered = compose::render_label(&loaded, font_options)
        .with_context(|| format!("failed to render label {}", loaded.path.display()))?;
    let document = export::export_rendered(&rendered)
        .with_context(|| format!("failed to generate geometry for {}", loaded.path.display()))?;
    let gridfinity_json = resolve_gridfinity_config(&args.remote, loaded.bin_path().as_deref())?;
    download_document_step(
        &document,
        args.remote,
        output,
        &format!("label {}", loaded.path.display()),
        &gridfinity_json,
    )
}

fn export_plate_step(args: ExportPlateArgs, font_options: &svg::FontOptions) -> Result<()> {
    let output = args
        .remote
        .output
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from("plate.step"));
    step::ensure_output_available(&output, args.remote.force)?;
    let plate = plate::build_plate(
        &args.labels,
        &args.dimensions,
        &args.column_gap,
        &args.row_gap,
        font_options,
    )
    .context("failed to generate label plate geometry")?;
    let gridfinity_json = resolve_gridfinity_config(&args.remote, None)?;
    download_document_step(
        &plate.document,
        args.remote,
        output,
        "label plate",
        &gridfinity_json,
    )
}

fn download_document_step(
    document: &export::ExportDocument,
    remote: RemoteExportArgs,
    output: std::path::PathBuf,
    description: &str,
    gridfinity_json: &str,
) -> Result<()> {
    let label_json = serde_json::to_string(document)
        .context("failed to serialize label geometry for Onshape")?;

    let credentials = credentials::Credentials::load(remote.onshape_credentials)?;
    let target = onshape::ModelTarget::parse(&remote.onshape_model)?;
    let client = onshape::OnshapeClient::new(credentials)?;
    let destination_name = output
        .file_stem()
        .and_then(|name| name.to_str())
        .context("STEP output must have a UTF-8 file name")?;
    let contents = client
        .export_label_step(&target, &label_json, gridfinity_json, destination_name)
        .with_context(|| format!("failed to export {description}"))?;
    step::validate_label_step(&contents, &document.filaments)
        .context("downloaded Onshape STEP failed label part validation")?;
    step::write_atomic(&output, &contents, remote.force)?;
    eprintln!(
        "{} {} ({} bytes)",
        "Finished".green().bold(),
        output.display(),
        contents.len()
    );
    Ok(())
}

fn validate_bin(path: &std::path::Path) -> Result<()> {
    let loaded = bin_config::LoadedBin::load(path)?;
    eprintln!(
        "{} {} is valid",
        "Finished".green().bold(),
        loaded.path.display()
    );
    Ok(())
}

fn inspect_bin(path: &std::path::Path) -> Result<()> {
    let loaded = bin_config::LoadedBin::load(path)?;
    let config = &loaded.config;
    println!(
        "{} {}",
        "Inspecting".green().bold(),
        loaded.path.display().to_string().bold()
    );
    print_inspect_field("type", "bin");
    print_inspect_field(
        "size",
        &format!(
            "{} × {} × {} units ({} × {} × {} mm)",
            config.size[0],
            config.size[1],
            config.size[2],
            config.size[0] * 42,
            config.size[1] * 42,
            config.size[2] * 7
        ),
    );
    print_inspect_field(
        "base",
        if config.base.enabled {
            "enabled"
        } else {
            "disabled"
        },
    );
    print_inspect_field(
        "bin",
        if config.bin.enabled {
            "enabled"
        } else {
            "disabled"
        },
    );
    print_inspect_field(
        "divider",
        &format!(
            "{} columns × {} rows, {} merges",
            config.divider.columns.len(),
            config.divider.rows.len(),
            config.divider.merges.len()
        ),
    );
    print_inspect_field("easy-grab scoops", &config.easy_grab_count()?.to_string());
    print_inspect_field(
        "label supports",
        if config.supports_enabled()? {
            "enabled"
        } else {
            "disabled"
        },
    );
    print_inspect_field(
        "parts",
        &config
            .expected_parts(bin_config::BinComponent::All)?
            .join(", "),
    );
    Ok(())
}

fn export_bin_step(args: BinExportArgs) -> Result<()> {
    let output = args
        .output
        .clone()
        .unwrap_or_else(|| default_step_path(&args.bin));
    step::ensure_output_available(&output, args.force)?;
    if let Some(image) = &args.image {
        if image == &output {
            anyhow::bail!("STEP and PNG outputs must use different paths");
        }
        step::ensure_output_available(image, args.force)?;
    }
    let loaded = bin_config::LoadedBin::load(&args.bin)?;
    let gridfinity_json = loaded.config.canonical_json(args.component)?;
    let expected_parts = loaded.config.expected_parts(args.component)?;
    let credentials = credentials::Credentials::load(args.onshape_credentials)?;
    let target = onshape::ModelTarget::parse(&args.onshape_model)?;
    let client = onshape::OnshapeClient::new(credentials)?;
    let destination_name = output
        .file_stem()
        .and_then(|name| name.to_str())
        .context("STEP output must have a UTF-8 file name")?;
    let contents = client
        .export_bin_step(&target, &gridfinity_json, destination_name)
        .with_context(|| format!("failed to export bin {}", loaded.path.display()))?;
    step::validate_bin_step(&contents, &expected_parts)
        .context("downloaded Onshape STEP failed bin part validation")?;
    let preview = args
        .image
        .as_ref()
        .map(|_| client.render_bin_preview(&target, &gridfinity_json))
        .transpose()
        .with_context(|| format!("failed to render bin preview for {}", loaded.path.display()))?;

    step::write_atomic(&output, &contents, args.force)?;
    eprintln!(
        "{} {} ({} bytes)",
        "Finished".green().bold(),
        output.display(),
        contents.len()
    );
    if let (Some(path), Some(preview)) = (args.image, preview) {
        step::write_atomic(&path, &preview, args.force)?;
        eprintln!(
            "{} {} ({} bytes)",
            "Finished".green().bold(),
            path.display(),
            preview.len()
        );
    }
    Ok(())
}

fn default_step_path(input: &std::path::Path) -> std::path::PathBuf {
    let stem = input
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("export");
    std::path::PathBuf::from(format!("{stem}.step"))
}

fn resolve_gridfinity_config(
    remote: &RemoteExportArgs,
    fallback_bin: Option<&std::path::Path>,
) -> Result<String> {
    if let Some(path) = &remote.gridfinity_config {
        return read_gridfinity_config(path);
    }
    let bin = remote.bin.as_deref().or(fallback_bin).context(
        "label export requires --bin PATH, a label `bin` field, or legacy --gridfinity-config PATH",
    )?;
    let loaded = bin_config::LoadedBin::load(bin)
        .with_context(|| format!("failed to load label prototype bin {}", bin.display()))?;
    loaded
        .config
        .canonical_json(bin_config::BinComponent::All)
        .with_context(|| format!("failed to serialize label prototype bin {}", bin.display()))
}

fn read_gridfinity_config(path: &std::path::Path) -> Result<String> {
    let source = std::fs::read_to_string(path).with_context(|| {
        format!(
            "failed to read Gridfinity Ultimate configuration {}",
            path.display()
        )
    })?;
    let value: serde_json::Value = serde_json::from_str(&source).with_context(|| {
        format!(
            "failed to parse Gridfinity Ultimate configuration {} as JSON",
            path.display()
        )
    })?;
    if !value.is_object() {
        anyhow::bail!(
            "Gridfinity Ultimate configuration {} must be a JSON object",
            path.display()
        );
    }
    serde_json::to_string(&value).with_context(|| {
        format!(
            "failed to serialize Gridfinity Ultimate configuration {}",
            path.display()
        )
    })
}

fn validate_labels(
    label: Option<&std::path::Path>,
    root: Option<&std::path::Path>,
    font_options: &svg::FontOptions,
) -> Result<()> {
    let labels: Vec<(String, std::path::PathBuf)> = if let Some(label) = label {
        vec![(label.display().to_string(), label.to_owned())]
    } else {
        let (root, paths) = config::discover_labels(root)?;
        paths
            .into_iter()
            .map(|path| {
                let display = path
                    .strip_prefix(&root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                (display, path)
            })
            .collect()
    };

    let total = labels.len();
    let mut valid = 0usize;
    for (display, path) in labels {
        let result = config::LoadedLabel::load(&path)
            .with_context(|| format!("failed to load label {}", path.display()))
            .and_then(|loaded| {
                compose::render_label_svg(&loaded, font_options)
                    .with_context(|| format!("failed to validate label {}", loaded.path.display()))
            });
        match result {
            Ok(_) => valid += 1,
            Err(error) => {
                eprintln!("{} {}", "error:".red().bold(), display.bold());
                eprintln!("  {error:#}");
            }
        }
    }

    let summary = format!("{valid}/{total} valid").bold();
    if valid == total {
        eprintln!("{} {summary}", "Finished".green().bold());
    } else {
        eprintln!("{} {summary}", "Failed".red().bold());
        std::process::exit(1);
    }
    Ok(())
}

fn inspect_file(
    path: &std::path::Path,
    font_options: &svg::FontOptions,
    preview_options: terminal_preview::PreviewOptions,
    preview: bool,
) -> Result<()> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    println!(
        "{} {}",
        "Inspecting".green().bold(),
        path.display().to_string().bold()
    );
    match extension.as_str() {
        "toml" => inspect_label(path, font_options, preview_options, preview),
        "svg" => inspect_svg(path, font_options, preview_options, preview),
        _ => anyhow::bail!("inspect supports label TOML and template/icon SVG files"),
    }
}

fn inspect_label(
    path: &std::path::Path,
    font_options: &svg::FontOptions,
    preview_options: terminal_preview::PreviewOptions,
    preview: bool,
) -> Result<()> {
    let label = config::LoadedLabel::load(path)
        .with_context(|| format!("failed to load label {}", path.display()))?;
    let template = template::TemplateInfo::load(&label.template_path())?;
    let filaments = compose::label_filaments(&label)?;
    print_inspect_field("type", "label");
    print_inspect_field("template", &label.template_path().display().to_string());
    print_inspect_field(
        "size",
        &format!(
            "{} × {} mm",
            compact(template.width_mm),
            compact(template.height_mm)
        ),
    );
    print_inspect_field("base filament", &label.config.filament.to_string());
    if let Some(bin) = label.bin_path() {
        print_inspect_field("bin", &bin.display().to_string());
    }
    print_inspect_field(
        "filaments",
        &filaments
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", "),
    );
    if label.config.text.is_empty() {
        print_inspect_field("text", "(none)");
    } else {
        for (name, value) in &label.config.text {
            print_inspect_field(&format!("text.{name}"), &value.content);
        }
    }
    for (name, values) in &label.config.icons {
        print_inspect_field(&format!("icons.{name}"), &format!("{} items", values.len()));
    }
    if preview {
        let rendered = compose::render_label_svg(&label, font_options)
            .with_context(|| format!("failed to preview label {}", path.display()))?;
        show_inspect_svg(&rendered, &path.display().to_string(), preview_options)?;
    }
    Ok(())
}

fn inspect_svg(
    path: &std::path::Path,
    font_options: &svg::FontOptions,
    preview_options: terminal_preview::PreviewOptions,
    preview: bool,
) -> Result<()> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read SVG {}", path.display()))?;
    let info = template::TemplateInfo::parse(&source)
        .with_context(|| format!("invalid template or icon SVG {}", path.display()))?;
    let colors = color::ColorMapping::load(path)?;
    let kind = if info.text_fields.is_empty() && info.icon_boxes.is_empty() {
        "icon"
    } else {
        "template"
    };
    print_inspect_field("type", kind);
    print_inspect_field(
        "size",
        &format!(
            "{} × {} mm",
            compact(info.width_mm),
            compact(info.height_mm)
        ),
    );
    print_inspect_field(
        "viewBox",
        &format!(
            "{} {} {} {}",
            compact(info.view_box.min_x),
            compact(info.view_box.min_y),
            compact(info.view_box.width),
            compact(info.view_box.height)
        ),
    );
    for (name, default) in &info.text_fields {
        print_inspect_field(&format!("text.{name}"), default);
    }
    for (name, icon_box) in &info.icon_boxes {
        let direction = match icon_box.direction {
            template::IconDirection::Horizontal => "horizontal",
            template::IconDirection::Vertical => "vertical",
        };
        let alignment = match (icon_box.direction, icon_box.alignment) {
            (_, template::IconAlignment::Center) => "center",
            (template::IconDirection::Horizontal, template::IconAlignment::Start) => "left",
            (template::IconDirection::Horizontal, template::IconAlignment::End) => "right",
            (template::IconDirection::Vertical, template::IconAlignment::Start) => "top",
            (template::IconDirection::Vertical, template::IconAlignment::End) => "bottom",
        };
        print_inspect_field(
            &format!("icons.{name}"),
            &format!(
                "x={} y={} width={} height={}, {direction}, {alignment}",
                compact(icon_box.x),
                compact(icon_box.y),
                compact(icon_box.width),
                compact(icon_box.height)
            ),
        );
    }
    for (source, filament) in &colors.source_to_filament {
        print_inspect_field(&format!("color #{source}"), &format!("filament {filament}"));
    }
    print_inspect_field(
        "color mapping",
        &colors
            .sidecar
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "automatic".to_owned()),
    );
    if preview {
        let parser = svg::SvgParser::new(font_options);
        let tree = parser.parse(
            &source,
            path.parent().unwrap_or_else(|| std::path::Path::new(".")),
        )?;
        let mut session = terminal_preview::PreviewSession::new(preview_options)?;
        session.show_tree(&tree, &path.display().to_string(), false)?;
    }
    Ok(())
}

fn show_inspect_svg(
    svg: &str,
    label: &str,
    preview_options: terminal_preview::PreviewOptions,
) -> Result<()> {
    let mut session = terminal_preview::PreviewSession::new(preview_options)?;
    session.show_svg(svg, label, false)?;
    Ok(())
}

fn print_inspect_field(name: &str, value: &str) {
    println!("  {} {value}", format!("{name}:").dimmed());
}

fn compact(value: f64) -> String {
    let value = format!("{value:.6}");
    value.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn try_preview(svg: &str, label: &str, options: terminal_preview::PreviewOptions, clear: bool) {
    if let Err(error) = terminal_preview::show_svg(svg, label, options, clear) {
        eprintln!("{} {error:#}", "terminal preview failed:".yellow().bold());
    }
}
