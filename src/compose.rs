use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};

use anyhow::{Context, Result};
use xmltree::{Element, EmitterConfig, XMLNode};

use crate::{
    config::{IconPlacement, LoadedLabel, parse_length_mm},
    layout::RowItem,
    template::TemplateInfo,
    text::{TextRun, parse_colored_text},
};

pub struct RenderedLabel {
    pub svg: String,
    pub palette: crate::color::PreviewPalette,
    pub size_mm: [f64; 2],
}

pub fn render_label_svg(label: &LoadedLabel, system_fonts: bool) -> Result<String> {
    Ok(render_label(label, system_fonts)?.svg)
}

pub fn render_label(label: &LoadedLabel, system_fonts: bool) -> Result<RenderedLabel> {
    let filaments = label_filaments(label)
        .with_context(|| format!("failed to collect filaments for {}", label.path.display()))?;
    let palette =
        crate::color::PreviewPalette::new(filaments).context("failed to create preview palette")?;
    render_label_with_palette(label, system_fonts, palette)
}

pub fn label_filaments(label: &LoadedLabel) -> Result<BTreeSet<u32>> {
    label
        .validate()
        .with_context(|| format!("invalid label {}", label.path.display()))?;
    let template_colors =
        crate::color::ColorMapping::load(&label.template_path()).with_context(|| {
            format!(
                "failed to load colors for {}",
                label.template_path().display()
            )
        })?;
    collect_filaments(label, &template_colors)
}

pub fn render_label_with_palette(
    label: &LoadedLabel,
    system_fonts: bool,
    palette: crate::color::PreviewPalette,
) -> Result<RenderedLabel> {
    label
        .validate()
        .with_context(|| format!("invalid label {}", label.path.display()))?;
    let template_path = label.template_path();
    let source = fs::read_to_string(&template_path)
        .with_context(|| format!("failed to read template {}", template_path.display()))?;
    let template = TemplateInfo::load(&template_path)
        .with_context(|| format!("failed to load template {}", template_path.display()))?;
    let template_colors = crate::color::ColorMapping::load(&template_path).with_context(|| {
        format!(
            "failed to load template colors for {}",
            template_path.display()
        )
    })?;
    let recolored_template =
        crate::color::recolor_svg(&source, &template_colors.source_to_filament, &palette);
    let mut root = Element::parse(recolored_template.as_bytes()).context("invalid template XML")?;
    let svg_parser = crate::svg::SvgParser::new(&label.project_root, system_fonts);

    apply_text_fields(&mut root, label, &palette)
        .with_context(|| format!("failed to compose template {}", template_path.display()))?;
    let mut icons = compose_icons(label, &svg_parser, &palette)
        .with_context(|| format!("failed to compose icons for {}", label.path.display()))?;
    replace_icon_boxes(&mut root, &mut icons);
    let composed = serialize_element(&root).context("failed to serialize composed label")?;

    let svg = crate::svg::normalize_svg_with_parser(
        &composed,
        template_path.parent().expect("template has a parent"),
        &svg_parser,
        None,
    )
    .with_context(|| format!("failed to normalize template {}", template_path.display()))?;
    Ok(RenderedLabel {
        svg,
        palette,
        size_mm: [template.width_mm, template.height_mm],
    })
}

fn collect_filaments(
    label: &LoadedLabel,
    template_colors: &crate::color::ColorMapping,
) -> Result<BTreeSet<u32>> {
    let mut filaments: BTreeSet<u32> = template_colors
        .source_to_filament
        .values()
        .copied()
        .collect();
    for (field, value) in &label.config.text {
        filaments.extend(
            parse_colored_text(&value.content)
                .with_context(|| format!("invalid colored text in field {field:?}"))?
                .into_iter()
                .map(|run| run.filament),
        );
    }
    for entries in label.config.icons.values() {
        for entry in entries {
            let IconPlacement::Icon { icon } = entry else {
                continue;
            };
            let resolved = label
                .resolve_icon(icon)
                .with_context(|| format!("failed to resolve icon {icon:?}"))?;
            filaments.extend(
                resolved
                    .color_mapping()
                    .with_context(|| {
                        format!(
                            "failed to load colors for icon {icon:?} at {}",
                            resolved.path.display()
                        )
                    })?
                    .source_to_filament
                    .values()
                    .copied(),
            );
        }
    }
    Ok(filaments)
}

fn compose_icons(
    label: &LoadedLabel,
    svg_parser: &crate::svg::SvgParser,
    palette: &crate::color::PreviewPalette,
) -> Result<BTreeMap<String, Vec<Element>>> {
    let template = TemplateInfo::load(&label.template_path()).with_context(|| {
        format!(
            "failed to load template {}",
            label.template_path().display()
        )
    })?;
    let mut result = BTreeMap::new();
    let mut instance_index = 0usize;

    for (box_name, entries) in &label.config.icons {
        let mut box_result = Vec::new();
        let icon_box = template
            .icon_boxes
            .get(box_name)
            .with_context(|| format!("unknown icon box {box_name:?}"))?;
        let mut row = Vec::new();
        let mut icon_details = Vec::new();
        for entry in entries {
            match entry {
                IconPlacement::Icon { icon } => {
                    let resolved = label
                        .resolve_icon(icon)
                        .with_context(|| format!("failed to resolve icon {icon:?}"))?;
                    let info = TemplateInfo::load(&resolved.path).with_context(|| {
                        format!("failed to inspect icon {}", resolved.path.display())
                    })?;
                    row.push(RowItem::Icon {
                        name: icon.clone(),
                        aspect_ratio: info.view_box.width / info.view_box.height,
                    });
                    icon_details.push(resolved);
                }
                IconPlacement::Spacer { spacer } => row.push(RowItem::Spacer {
                    width: parse_length_mm(spacer)
                        .with_context(|| format!("invalid spacer in icon box {box_name:?}"))?
                        * template.view_box.width
                        / template.width_mm,
                }),
            }
        }

        let placed = crate::layout::layout_icon_row(
            icon_box.x,
            icon_box.y,
            icon_box.width,
            icon_box.height,
            &row,
        )
        .with_context(|| format!("failed to lay out icon box {box_name:?}"))?;
        for (placement, resolved) in placed.iter().zip(icon_details) {
            let source = fs::read_to_string(&resolved.path)
                .with_context(|| format!("failed to read icon {}", resolved.path.display()))?;
            let colors = resolved
                .color_mapping()
                .with_context(|| format!("invalid colors for icon {}", resolved.path.display()))?;
            let recolored = crate::color::recolor_svg(&source, &colors.source_to_filament, palette);
            let normalized = crate::svg::normalize_svg_with_parser(
                &recolored,
                resolved.path.parent().expect("icon has a parent"),
                svg_parser,
                Some(format!("icon-{instance_index}-")),
            )
            .with_context(|| format!("failed to normalize icon {}", resolved.path.display()))?;
            let mut normalized_root =
                Element::parse(normalized.as_bytes()).context("invalid normalized icon SVG")?;
            let normalized_width: f64 = normalized_root
                .attributes
                .get("width")
                .context("normalized icon has no width")?
                .parse()
                .context("normalized icon width is not numeric")?;
            let normalized_height: f64 = normalized_root
                .attributes
                .get("height")
                .context("normalized icon has no height")?
                .parse()
                .context("normalized icon height is not numeric")?;
            let inherited_fill = colors
                .source_to_filament
                .get("000000")
                .map(|filament| palette.color(*filament).to_owned())
                .unwrap_or_else(|| "000000".to_owned());

            let namespace = normalized_root.namespace.clone();
            let mut group = svg_element("g", namespace.clone());
            group
                .attributes
                .insert("fill".to_owned(), format!("#{inherited_fill}"));
            group.children = std::mem::take(&mut normalized_root.children);

            let mut nested_svg = svg_element("svg", namespace);
            nested_svg
                .attributes
                .insert("x".to_owned(), placement.x.to_string());
            nested_svg
                .attributes
                .insert("y".to_owned(), placement.y.to_string());
            nested_svg
                .attributes
                .insert("width".to_owned(), placement.width.to_string());
            nested_svg
                .attributes
                .insert("height".to_owned(), placement.height.to_string());
            nested_svg.attributes.insert(
                "viewBox".to_owned(),
                format!("0 0 {normalized_width} {normalized_height}"),
            );
            nested_svg.children.push(XMLNode::Element(group));
            box_result.push(nested_svg);
            instance_index += 1;
        }
        result.insert(box_name.clone(), box_result);
    }
    Ok(result)
}

/// Replace icon-box rectangles in place so their ancestor transforms remain in
/// effect. A transform on the rectangle itself is transferred to a wrapper;
/// final viewport scaling and affine flattening are left to usvg.
fn replace_icon_boxes(element: &mut Element, replacements: &mut BTreeMap<String, Vec<Element>>) {
    let mut children = Vec::with_capacity(element.children.len());
    for node in std::mem::take(&mut element.children) {
        let XMLNode::Element(mut child) = node else {
            children.push(node);
            continue;
        };
        let box_name = child
            .attributes
            .get("id")
            .and_then(|id| id.strip_prefix("icons-"))
            .map(str::to_owned);
        if let Some(box_name) = box_name {
            if let Some(replacement) = replacements.remove(&box_name)
                && !replacement.is_empty()
            {
                let mut group = svg_element("g", child.namespace.clone());
                for attribute in ["transform", "transform-origin", "transform-box"] {
                    if let Some(value) = child.attributes.get(attribute) {
                        group
                            .attributes
                            .insert(attribute.to_owned(), value.to_owned());
                    }
                }
                if let Some(style) = child.attributes.get("style")
                    && let Some(style) = transform_style(style)
                {
                    group.attributes.insert("style".to_owned(), style);
                }
                group.children = replacement.into_iter().map(XMLNode::Element).collect();
                children.push(XMLNode::Element(group));
            }
            continue;
        }
        replace_icon_boxes(&mut child, replacements);
        children.push(XMLNode::Element(child));
    }
    element.children = children;
}

fn transform_style(style: &str) -> Option<String> {
    let declarations = style
        .split(';')
        .filter_map(|declaration| {
            let (name, _) = declaration.split_once(':')?;
            matches!(
                name.trim(),
                "transform" | "transform-origin" | "transform-box"
            )
            .then(|| declaration.trim())
        })
        .collect::<Vec<_>>();
    (!declarations.is_empty()).then(|| declarations.join(";"))
}

fn apply_text_fields(
    element: &mut Element,
    label: &LoadedLabel,
    palette: &crate::color::PreviewPalette,
) -> Result<()> {
    if let Some(field) = element
        .attributes
        .get("id")
        .and_then(|id| id.strip_prefix("text-"))
        && let Some(value) = label.config.text.get(field)
    {
        let runs = parse_colored_text(&value.content)
            .with_context(|| format!("invalid colored text in field {field:?}"))?;
        if !replace_first_text_tspan(element, &runs, palette) {
            replace_with_runs(element, &runs, palette);
        }
        return Ok(());
    }

    for child in &mut element.children {
        if let Some(child) = child.as_mut_element() {
            apply_text_fields(child, label, palette)?;
        }
    }
    Ok(())
}

fn replace_first_text_tspan(
    element: &mut Element,
    runs: &[TextRun],
    palette: &crate::color::PreviewPalette,
) -> bool {
    for child in &mut element.children {
        let Some(child) = child.as_mut_element() else {
            continue;
        };
        if child.name == "tspan"
            && child
                .children
                .iter()
                .any(|node| matches!(node, XMLNode::Text(_) | XMLNode::CData(_)))
        {
            replace_with_runs(child, runs, palette);
            return true;
        }
        if replace_first_text_tspan(child, runs, palette) {
            return true;
        }
    }
    false
}

fn replace_with_runs(
    element: &mut Element,
    runs: &[TextRun],
    palette: &crate::color::PreviewPalette,
) {
    let namespace = element.namespace.clone();
    element.children = runs
        .iter()
        .map(|run| {
            let mut tspan = svg_element("tspan", namespace.clone());
            tspan.attributes.insert(
                "fill".to_owned(),
                format!("#{}", palette.color(run.filament)),
            );
            tspan.children.push(XMLNode::Text(run.text.clone()));
            XMLNode::Element(tspan)
        })
        .collect();
}

fn svg_element(name: &str, namespace: Option<String>) -> Element {
    let mut element = Element::new(name);
    element.namespace = namespace;
    element
}

fn serialize_element(element: &Element) -> Result<String> {
    let mut output = Vec::new();
    element
        .write_with_config(
            &mut output,
            EmitterConfig::new()
                .write_document_declaration(false)
                .perform_indent(false),
        )
        .context("failed to serialize composed SVG")?;
    String::from_utf8(output).context("composed SVG is not UTF-8")
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf};

    use super::*;
    use crate::config::{IconPlacement, LabelConfig, TextValue};

    fn label_with_text(content: &str) -> LoadedLabel {
        LoadedLabel {
            path: PathBuf::from("label.toml"),
            project_root: PathBuf::from("."),
            config: LabelConfig {
                template: "template.svg".to_owned(),
                text: BTreeMap::from([(
                    "main".to_owned(),
                    TextValue {
                        content: content.to_owned(),
                    },
                )]),
                icon: BTreeMap::new(),
                icons: BTreeMap::new(),
            },
        }
    }

    #[test]
    fn creates_text_nodes_and_removes_icon_boxes() {
        let mut root = Element::parse(
            br#"<svg xmlns="http://www.w3.org/2000/svg"><text id="text-main"><tspan x="1">old</tspan></text><g><rect id="icons-main"/></g></svg>"#
                .as_slice(),
        )
        .unwrap();
        let palette = crate::color::PreviewPalette::new([0, 1]).unwrap();
        apply_text_fields(&mut root, &label_with_text(r#"A{\<&\>}B"#), &palette).unwrap();
        replace_icon_boxes(&mut root, &mut BTreeMap::new());
        let output = serialize_element(&root).unwrap();

        assert!(!output.contains("icons-main"));
        assert!(output.contains("fill=\"#0000ff\""));
        assert!(output.contains("&lt;&amp;&gt;"));
        roxmltree::Document::parse(&output).unwrap();
    }

    #[test]
    fn transformed_icon_boxes_export_at_their_transformed_position() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::create_dir_all(root.join("icons")).unwrap();
        fs::write(
            root.join("templates/label.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100"><g transform="translate(20 30)"><rect id="icons-main" x="0" y="0" width="10" height="10" transform="scale(2)" fill="none"/></g></svg>"#,
        )
        .unwrap();
        fs::write(
            root.join("icons/square.svg"),
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="10mm" height="10mm" viewBox="0 0 10 10"><g transform="translate(1 1) scale(.8)"><path fill="#000000" d="M0 0H10V10H0Z"/></g></svg>"##,
        )
        .unwrap();
        let label = LoadedLabel::from_config(
            LabelConfig {
                template: "label.svg".to_owned(),
                text: BTreeMap::new(),
                icon: BTreeMap::new(),
                icons: BTreeMap::from([(
                    "main".to_owned(),
                    vec![IconPlacement::Icon {
                        icon: "icons/square.svg".to_owned(),
                    }],
                )]),
            },
            root.to_owned(),
        );

        let rendered = render_label(&label, false).unwrap();
        let exported = crate::export::export_rendered(&rendered).unwrap();
        let contour = &exported.parts[0].shapes[0].contours[0];
        let mut points = vec![contour.start];
        for segment in &contour.segments {
            match segment {
                crate::export::Segment::Line { to } => points.push(*to),
                crate::export::Segment::Cubic { c1, c2, to } => {
                    points.extend([*c1, *c2, *to]);
                }
            }
        }
        let bounds = points.iter().fold(
            [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ],
            |mut bounds, point| {
                bounds[0] = bounds[0].min(point[0]);
                bounds[1] = bounds[1].min(point[1]);
                bounds[2] = bounds[2].max(point[0]);
                bounds[3] = bounds[3].max(point[1]);
                bounds
            },
        );
        for (actual, expected) in bounds.into_iter().zip([-28.0, 2.0, -12.0, 18.0]) {
            assert!((actual - expected).abs() < 1e-5, "{bounds:?}");
        }
    }
}
