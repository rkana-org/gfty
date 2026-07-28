use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result, bail};
use roxmltree::Document;

use crate::config::parse_length_mm;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewBox {
    pub min_x: f64,
    pub min_y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemplateInfo {
    pub width_mm: f64,
    pub height_mm: f64,
    pub view_box: ViewBox,
    pub text_fields: BTreeMap<String, String>,
    pub icon_boxes: BTreeMap<String, IconBox>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IconBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl TemplateInfo {
    pub fn load(path: &Path) -> Result<Self> {
        let source = fs::read_to_string(path)
            .with_context(|| format!("failed to read template {}", path.display()))?;
        Self::parse(&source).with_context(|| format!("invalid template {}", path.display()))
    }

    pub fn parse(source: &str) -> Result<Self> {
        let document = Document::parse(source).context("invalid SVG XML")?;
        let root = document.root_element();
        if root.tag_name().name() != "svg" {
            bail!("root element must be <svg>");
        }

        let width = root
            .attribute("width")
            .context("template <svg> needs an explicit physical width")?;
        let height = root
            .attribute("height")
            .context("template <svg> needs an explicit physical height")?;
        let width_mm = parse_length_mm(width).context("invalid template width")?;
        let height_mm = parse_length_mm(height).context("invalid template height")?;
        if width_mm <= 0.0 || height_mm <= 0.0 {
            bail!("template width and height must be positive");
        }

        let view_box = parse_view_box(
            root.attribute("viewBox")
                .context("template <svg> needs an explicit viewBox")?,
        )?;

        let mut ids = BTreeMap::new();
        let mut text_fields = BTreeMap::new();
        let mut icon_boxes = BTreeMap::new();

        for node in document.descendants().filter(|node| node.is_element()) {
            let Some(id) = node.attribute("id") else {
                continue;
            };
            if let Some(previous) = ids.insert(id, node.range().start) {
                bail!(
                    "duplicate SVG id {id:?} at bytes {previous} and {}",
                    node.range().start
                );
            }

            if let Some(name) = id.strip_prefix("text-") {
                if name.is_empty() {
                    bail!("text field id must contain a name after `text-`");
                }
                if node.tag_name().name() != "text" {
                    bail!("configurable element {id:?} must be a <text> element");
                }
                let default_text = node
                    .descendants()
                    .filter(|child| child.is_text())
                    .filter_map(|child| child.text())
                    .collect();
                text_fields.insert(name.to_owned(), default_text);
            }

            if let Some(name) = id.strip_prefix("icons-") {
                if name.is_empty() {
                    bail!("icon box id must contain a name after `icons-`");
                }
                if node.tag_name().name() != "rect" {
                    bail!("icon box {id:?} must be a <rect> element");
                }
                icon_boxes.insert(name.to_owned(), parse_icon_box(node)?);
            }
        }

        Ok(Self {
            width_mm,
            height_mm,
            view_box,
            text_fields,
            icon_boxes,
        })
    }
}

fn parse_view_box(value: &str) -> Result<ViewBox> {
    let numbers: Vec<f64> = value
        .split(|c: char| c.is_ascii_whitespace() || c == ',')
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<f64>()
                .with_context(|| format!("invalid viewBox number {part:?}"))
        })
        .collect::<Result<_>>()?;
    if numbers.len() != 4 {
        bail!("viewBox must contain exactly four numbers");
    }
    if numbers.iter().any(|number| !number.is_finite()) || numbers[2] <= 0.0 || numbers[3] <= 0.0 {
        bail!("viewBox width and height must be finite and positive");
    }
    Ok(ViewBox {
        min_x: numbers[0],
        min_y: numbers[1],
        width: numbers[2],
        height: numbers[3],
    })
}

fn parse_icon_box(node: roxmltree::Node<'_, '_>) -> Result<IconBox> {
    let number = |name: &str, default: Option<f64>| -> Result<f64> {
        match node.attribute(name) {
            Some(value) => value
                .parse::<f64>()
                .with_context(|| format!("icon box has invalid {name}={value:?}")),
            None => default.with_context(|| format!("icon box is missing {name}")),
        }
    };
    let result = IconBox {
        x: number("x", Some(0.0))?,
        y: number("y", Some(0.0))?,
        width: number("width", None)?,
        height: number("height", None)?,
    };
    if result.width <= 0.0 || result.height <= 0.0 {
        bail!("icon box width and height must be positive");
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str = r#"
        <svg xmlns="http://www.w3.org/2000/svg" width="42mm" height="21mm" viewBox="0 0 84 42">
          <text id="text-main" x="42" y="20">Default</text>
          <rect id="icons-fasteners" x="4" y="24" width="76" height="14"/>
        </svg>
    "#;

    #[test]
    fn discovers_contract_elements() {
        let info = TemplateInfo::parse(TEMPLATE).unwrap();
        assert_eq!(info.width_mm, 42.0);
        assert_eq!(info.height_mm, 21.0);
        assert_eq!(info.text_fields["main"], "Default");
        assert_eq!(
            info.icon_boxes["fasteners"],
            IconBox {
                x: 4.0,
                y: 24.0,
                width: 76.0,
                height: 14.0
            }
        );
    }

    #[test]
    fn rejects_wrong_special_elements() {
        let bad = TEMPLATE.replace("<rect id=\"icons-fasteners\"", "<g id=\"icons-fasteners\"");
        assert!(TemplateInfo::parse(&bad).is_err());
    }

    #[test]
    fn rejects_duplicate_ids() {
        let bad = TEMPLATE.replace("</svg>", "<g id=\"text-main\"/></svg>");
        assert!(TemplateInfo::parse(&bad).is_err());
    }
}
