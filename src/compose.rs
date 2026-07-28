use std::fs;

use anyhow::{Context, Result, bail};
use roxmltree::Document;

use crate::{
    config::{IconPlacement, LoadedLabel, parse_length_mm},
    layout::RowItem,
    template::TemplateInfo,
    text::parse_colored_text,
};

#[derive(Debug)]
struct Replacement {
    start: usize,
    end: usize,
    value: String,
}

pub fn render_label_svg(label: &LoadedLabel, system_fonts: bool) -> Result<String> {
    label.validate()?;
    let template_path = label.template_path();
    let source = fs::read_to_string(&template_path)
        .with_context(|| format!("failed to read template {}", template_path.display()))?;
    let mut composed = compose_text_and_remove_boxes(&source, label)?;
    let icons = compose_icons(label, system_fonts)?;
    if !icons.is_empty() {
        let insertion = composed
            .rfind("</svg>")
            .context("template has no closing svg element")?;
        composed.insert_str(insertion, &icons);
    }
    crate::svg::normalize_svg(
        &composed,
        template_path.parent().expect("template has a parent"),
        &label.project_root,
        system_fonts,
    )
}

fn compose_icons(label: &LoadedLabel, system_fonts: bool) -> Result<String> {
    let template = TemplateInfo::load(&label.template_path())?;
    let mut markup = String::new();
    let mut instance_index = 0usize;

    for (box_name, entries) in &label.config.icons {
        let icon_box = template
            .icon_boxes
            .get(box_name)
            .with_context(|| format!("unknown icon box {box_name:?}"))?;
        let mut row = Vec::new();
        let mut icon_details = Vec::new();
        for entry in entries {
            match entry {
                IconPlacement::Icon { icon } => {
                    let definition = label
                        .config
                        .icon
                        .get(icon)
                        .with_context(|| format!("unknown icon alias {icon:?}"))?;
                    let path = label.icon_path(definition);
                    let info = TemplateInfo::load(&path)?;
                    row.push(RowItem::Icon {
                        name: icon.clone(),
                        aspect_ratio: info.view_box.width / info.view_box.height,
                    });
                    icon_details.push((definition, path));
                }
                IconPlacement::Spacer { spacer } => row.push(RowItem::Spacer {
                    width: parse_length_mm(spacer)? * template.view_box.width / template.width_mm,
                }),
            }
        }

        let placed = crate::layout::layout_icon_row(
            icon_box.x,
            icon_box.y,
            icon_box.width,
            icon_box.height,
            &row,
        )?;
        for (placement, (definition, path)) in placed.iter().zip(icon_details) {
            let source = fs::read_to_string(&path)
                .with_context(|| format!("failed to read icon {}", path.display()))?;
            let colors = crate::color::ColorMapping::load(&path)?
                .with_overrides(&definition.colors)
                .with_context(|| format!("invalid colors for icon {}", path.display()))?;
            let recolored = crate::color::recolor_svg(&source, &colors.source_to_filament);
            let normalized = crate::svg::normalize_svg_with_prefix(
                &recolored,
                path.parent().expect("icon has a parent"),
                &label.project_root,
                system_fonts,
                Some(format!("icon-{instance_index}-")),
            )?;
            let document = Document::parse(&normalized).context("invalid normalized icon SVG")?;
            let root = document.root_element();
            let normalized_width: f64 = root
                .attribute("width")
                .context("normalized icon has no width")?
                .parse()
                .context("normalized icon width is not numeric")?;
            let normalized_height: f64 = root
                .attribute("height")
                .context("normalized icon has no height")?
                .parse()
                .context("normalized icon height is not numeric")?;
            let (start, end) = inner_range(&normalized, root)?;
            let inner = &normalized[start..end];
            let inherited_fill = colors
                .source_to_filament
                .get("000000")
                .map(|filament| crate::color::filament_preview_color(*filament))
                .unwrap_or_else(|| "000000".to_owned());
            markup.push_str(&format!(
                "<svg x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {normalized_width} {normalized_height}\"><g fill=\"#{inherited_fill}\">{inner}</g></svg>",
                placement.x, placement.y, placement.width, placement.height
            ));
            instance_index += 1;
        }
    }
    Ok(markup)
}

fn compose_text_and_remove_boxes(source: &str, label: &LoadedLabel) -> Result<String> {
    let document = Document::parse(source).context("invalid template XML")?;
    let mut replacements = Vec::new();

    for node in document.descendants().filter(|node| node.is_element()) {
        let Some(id) = node.attribute("id") else {
            continue;
        };
        if let Some(field) = id.strip_prefix("text-")
            && let Some(value) = label.config.text.get(field)
        {
            let target = node
                .descendants()
                .find(|child| {
                    child.is_element()
                        && child.tag_name().name() == "tspan"
                        && child.children().any(|grandchild| grandchild.is_text())
                })
                .unwrap_or(node);
            let (start, end) = inner_range(source, target)?;
            let runs = parse_colored_text(&value.content)?;
            let mut markup = String::new();
            for run in runs {
                markup.push_str(&format!(
                    "<tspan fill=\"#{}\">{}</tspan>",
                    crate::color::filament_preview_color(run.filament),
                    escape_xml(&run.text)
                ));
            }
            replacements.push(Replacement {
                start,
                end,
                value: markup,
            });
        }

        if id.starts_with("icons-") {
            let range = node.range();
            replacements.push(Replacement {
                start: range.start,
                end: range.end,
                value: String::new(),
            });
        }
    }

    replacements.sort_by_key(|replacement| std::cmp::Reverse(replacement.start));
    let mut result = source.to_owned();
    let mut previous_start = source.len();
    for replacement in replacements {
        if replacement.end > previous_start {
            bail!(
                "overlapping template replacements around byte {}",
                replacement.start
            );
        }
        result.replace_range(replacement.start..replacement.end, &replacement.value);
        previous_start = replacement.start;
    }
    Ok(result)
}

fn inner_range(source: &str, node: roxmltree::Node<'_, '_>) -> Result<(usize, usize)> {
    let range = node.range();
    let fragment = &source[range.clone()];
    let open_end = fragment
        .find('>')
        .with_context(|| format!("malformed element at byte {}", range.start))?;
    let close_start = fragment.rfind("</").with_context(|| {
        format!(
            "text element at byte {} cannot be self-closing",
            range.start
        )
    })?;
    Ok((range.start + open_end + 1, range.start + close_start))
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_inner_ranges() {
        let source = r#"<svg><text id="text-x"><tspan x="1">old</tspan></text></svg>"#;
        let document = Document::parse(source).unwrap();
        let tspan = document
            .descendants()
            .find(|node| node.has_tag_name("tspan"))
            .unwrap();
        let (start, end) = inner_range(source, tspan).unwrap();
        assert_eq!(&source[start..end], "old");
    }

    #[test]
    fn escapes_xml_text() {
        assert_eq!(escape_xml("<&>\"'"), "&lt;&amp;&gt;&quot;&apos;");
    }
}
