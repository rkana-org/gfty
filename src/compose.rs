use std::fs;

use anyhow::{Context, Result, bail};
use roxmltree::Document;

use crate::{config::LoadedLabel, text::parse_colored_text};

#[derive(Debug)]
struct Replacement {
    start: usize,
    end: usize,
    value: String,
}

pub fn render_label_svg(label: &LoadedLabel, system_fonts: bool) -> Result<String> {
    label.validate()?;
    if label.config.icons.values().any(|items| !items.is_empty()) {
        bail!("icon composition is not implemented yet");
    }

    let template_path = label.template_path();
    let source = fs::read_to_string(&template_path)
        .with_context(|| format!("failed to read template {}", template_path.display()))?;
    let composed = compose_text_and_remove_boxes(&source, label)?;
    crate::svg::normalize_svg(
        &composed,
        template_path.parent().expect("template has a parent"),
        &label.project_root,
        system_fonts,
    )
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
                    preview_color(run.filament),
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

fn preview_color(filament: u32) -> String {
    const COLORS: [&str; 8] = [
        "000000", "0000ff", "00a000", "ff0000", "ff00ff", "00c0c0", "ff8000", "808080",
    ];
    COLORS
        .get(filament as usize)
        .map(|value| (*value).to_owned())
        .unwrap_or_else(|| format!("{:06x}", filament.wrapping_mul(2_654_435_761) & 0x00ff_ffff))
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
