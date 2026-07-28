use std::{collections::BTreeSet, fs};

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
    let palette = crate::color::PreviewPalette::new(label_filaments(label)?)?;
    render_label_with_palette(label, system_fonts, palette)
}

pub fn label_filaments(label: &LoadedLabel) -> Result<BTreeSet<u32>> {
    label.validate()?;
    let template_colors = crate::color::ColorMapping::load(&label.template_path())?;
    collect_filaments(label, &template_colors)
}

pub fn render_label_with_palette(
    label: &LoadedLabel,
    system_fonts: bool,
    palette: crate::color::PreviewPalette,
) -> Result<RenderedLabel> {
    label.validate()?;
    let template_path = label.template_path();
    let source = fs::read_to_string(&template_path)
        .with_context(|| format!("failed to read template {}", template_path.display()))?;
    let template = TemplateInfo::load(&template_path)?;
    let template_colors = crate::color::ColorMapping::load(&template_path)?;
    let recolored_template =
        crate::color::recolor_svg(&source, &template_colors.source_to_filament, &palette);
    let mut root = Element::parse(recolored_template.as_bytes()).context("invalid template XML")?;

    compose_text_and_remove_boxes(&mut root, label, &palette)?;
    root.children.extend(
        compose_icons(label, system_fonts, &palette)?
            .into_iter()
            .map(XMLNode::Element),
    );
    let composed = serialize_element(&root)?;

    let svg = crate::svg::normalize_svg(
        &composed,
        template_path.parent().expect("template has a parent"),
        &label.project_root,
        system_fonts,
    )?;
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
    for value in label.config.text.values() {
        filaments.extend(
            parse_colored_text(&value.content)?
                .into_iter()
                .map(|run| run.filament),
        );
    }
    for definition in label.config.icon.values() {
        filaments.extend(
            crate::color::ColorMapping::load(&label.icon_path(definition))?
                .with_overrides(&definition.colors)?
                .source_to_filament
                .values()
                .copied(),
        );
    }
    Ok(filaments)
}

fn compose_icons(
    label: &LoadedLabel,
    system_fonts: bool,
    palette: &crate::color::PreviewPalette,
) -> Result<Vec<Element>> {
    let template = TemplateInfo::load(&label.template_path())?;
    let mut result = Vec::new();
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
            let recolored = crate::color::recolor_svg(&source, &colors.source_to_filament, palette);
            let normalized = crate::svg::normalize_svg_with_prefix(
                &recolored,
                path.parent().expect("icon has a parent"),
                &label.project_root,
                system_fonts,
                Some(format!("icon-{instance_index}-")),
            )?;
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
            result.push(nested_svg);
            instance_index += 1;
        }
    }
    Ok(result)
}

fn compose_text_and_remove_boxes(
    root: &mut Element,
    label: &LoadedLabel,
    palette: &crate::color::PreviewPalette,
) -> Result<()> {
    remove_icon_boxes(root);
    apply_text_fields(root, label, palette)
}

fn remove_icon_boxes(element: &mut Element) {
    element.children.retain(|node| {
        node.as_element()
            .and_then(|child| child.attributes.get("id"))
            .is_none_or(|id| !id.starts_with("icons-"))
    });
    for child in &mut element.children {
        if let Some(child) = child.as_mut_element() {
            remove_icon_boxes(child);
        }
    }
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
        let runs = parse_colored_text(&value.content)?;
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
    use crate::config::{LabelConfig, TextValue};

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
        compose_text_and_remove_boxes(&mut root, &label_with_text(r#"A{\<&\>}B"#), &palette)
            .unwrap();
        let output = serialize_element(&root).unwrap();

        assert!(!output.contains("icons-main"));
        assert!(output.contains("fill=\"#0000ff\""));
        assert!(output.contains("&lt;&amp;&gt;"));
        roxmltree::Document::parse(&output).unwrap();
    }
}
