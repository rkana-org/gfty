mod cli;
mod color;
mod compose;
mod config;
mod credentials;
mod export;
mod layout;
mod onshape;
mod plate;
mod quick;
mod step;
mod svg;
mod template;
mod terminal_preview;
mod text;
mod watch;

use std::io::Write;

use anyhow::{Context, Result};
use clap::Parser;
use colored::Colorize;

use crate::cli::{Cli, Command, ExportArgs, ExportPlateArgs, RemoteExportArgs};

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
    let list_root = cli.root;
    let inspect_preview = cli.preview;
    let preview_options = terminal_preview::PreviewOptions {
        mode: cli.terminal_preview,
        width: cli.terminal_preview_width,
    };
    match cli.command {
        Command::Validate { label } => {
            validate_labels(label.as_deref(), list_root.as_deref(), &font_options)?;
        }
        Command::Render { label, output } => {
            let loaded = config::LoadedLabel::load(&label)
                .with_context(|| format!("failed to load label {}", label.display()))?;
            let svg = compose::render_label_svg(&loaded, &font_options)
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
            } else {
                let shown = terminal_preview::show_svg(
                    &svg,
                    &loaded.path.display().to_string(),
                    preview_options,
                    false,
                )
                .context("failed to render SVG in the terminal")?;
                if !shown {
                    anyhow::bail!(
                        "terminal preview is unavailable; use --output PATH or select a supported terminal preview mode"
                    );
                }
            }
        }
        Command::Build { label, output } => {
            let loaded = config::LoadedLabel::load(&label)
                .with_context(|| format!("failed to load label {}", label.display()))?;
            let rendered = compose::render_label(&loaded, &font_options)
                .with_context(|| format!("failed to render label {}", loaded.path.display()))?;
            std::fs::create_dir_all(&output).with_context(|| {
                format!("failed to create output directory {}", output.display())
            })?;
            std::fs::write(output.join("label.svg"), &rendered.svg)
                .with_context(|| format!("failed to write label SVG in {}", output.display()))?;
            let document = export::export_rendered(&rendered)
                .with_context(|| format!("failed to export label {}", loaded.path.display()))?;
            write_json(&output.join("label.json"), &document)?;
        }
        Command::Export(args) => export_label_step(args, &font_options)?,
        Command::ExportPlate(args) => export_plate_step(args, &font_options)?,
        Command::Quick {
            template,
            filament,
            text,
            icon,
            svg,
            save,
            json,
        } => {
            if svg.is_none() && json.is_none() && save.is_none() {
                anyhow::bail!("quick needs at least one of --svg, --json, or --save");
            }
            let loaded = quick::build_quick_label(&template, filament, &text, &icon)
                .context("failed to build quick label configuration")?;
            let rendered = compose::render_label(&loaded, &font_options)
                .context("failed to render quick label")?;
            try_preview(&rendered.svg, "quick label", preview_options, false);
            if let Some(output) = save {
                quick::save_quick_label(&loaded, &output)?;
            }
            if let Some(output) = svg {
                std::fs::write(&output, &rendered.svg)
                    .with_context(|| format!("failed to write SVG {}", output.display()))?;
            }
            if let Some(output) = json {
                let document =
                    export::export_rendered(&rendered).context("failed to export quick label")?;
                write_json(&output, &document)?;
            }
        }
        Command::Inspect { file } => {
            inspect_file(&file, &font_options, preview_options, inspect_preview)?;
        }
        Command::Plate {
            dimensions,
            column_gap,
            row_gap,
            svg,
            json,
            labels,
        } => {
            // With no explicit output option, mirror export and quick --json
            // by writing compact JSON to stdout.
            let json = if svg.is_none() && json.is_none() {
                Some(std::path::PathBuf::from("-"))
            } else {
                json
            };
            let output =
                plate::build_plate(&labels, &dimensions, &column_gap, &row_gap, &font_options)
                    .context("failed to generate plate")?;
            try_preview(&output.svg, "plate", preview_options, false);
            if let Some(path) = svg {
                std::fs::write(&path, output.svg)
                    .with_context(|| format!("failed to write plate SVG {}", path.display()))?;
            }
            if let Some(path) = json {
                write_json(&path, &output.document)?;
            }
        }
        Command::Watch { label, svg, json } => {
            watch::watch_label(
                &label,
                svg.as_deref(),
                json.as_deref(),
                &font_options,
                preview_options,
            )
            .with_context(|| format!("failed to watch label {}", label.display()))?;
        }
    }
    Ok(())
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
    download_document_step(
        &document,
        args.remote,
        output,
        &format!("label {}", loaded.path.display()),
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
    download_document_step(&plate.document, args.remote, output, "label plate")
}

fn download_document_step(
    document: &export::ExportDocument,
    remote: RemoteExportArgs,
    output: std::path::PathBuf,
    description: &str,
) -> Result<()> {
    let label_json = serde_json::to_string(document)
        .context("failed to serialize label geometry for Onshape")?;
    let gridfinity_json = read_gridfinity_config(&remote.gridfinity_config)?;

    let credentials = credentials::Credentials::load(remote.onshape_credentials)?;
    let target = onshape::ModelTarget::parse(&remote.onshape_model)?;
    let client = onshape::OnshapeClient::new(credentials)?;
    let destination_name = output
        .file_stem()
        .and_then(|name| name.to_str())
        .context("STEP output must have a UTF-8 file name")?;
    let contents = client
        .export_label_step(&target, &label_json, &gridfinity_json, destination_name)
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

fn default_step_path(label: &std::path::Path) -> std::path::PathBuf {
    let stem = label
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("label");
    std::path::PathBuf::from(format!("{stem}.step"))
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

fn write_json(path: &std::path::Path, document: &export::ExportDocument) -> Result<()> {
    let mut json = serde_json::to_vec(document).context("failed to serialize Onshape JSON")?;
    json.push(b'\n');
    if is_stdout_path(path) {
        std::io::stdout()
            .lock()
            .write_all(&json)
            .context("failed to write JSON to stdout")?;
    } else {
        std::fs::write(path, json)
            .with_context(|| format!("failed to write JSON {}", path.display()))?;
    }
    Ok(())
}

fn is_stdout_path(path: &std::path::Path) -> bool {
    path == std::path::Path::new("-")
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
