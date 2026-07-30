use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use roxmltree::Document;
use serde::Deserialize;

use crate::config::normalize_hex_color;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorMapping {
    pub source_to_filament: BTreeMap<String, u32>,
    pub sidecar: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ColorSidecar {
    colors: BTreeMap<String, u32>,
}

impl ColorMapping {
    pub fn load(svg_path: &Path) -> Result<Self> {
        let source = fs::read_to_string(svg_path)
            .with_context(|| format!("failed to read SVG {}", svg_path.display()))?;
        let colors = discover_colors(&source)
            .with_context(|| format!("failed to inspect SVG colors in {}", svg_path.display()))?;
        let sidecar = svg_path.with_extension("toml");
        if sidecar.is_file() {
            Self::from_sidecar(&colors, &sidecar)
                .with_context(|| format!("invalid color sidecar {}", sidecar.display()))
        } else {
            Ok(Self {
                source_to_filament: colors
                    .into_iter()
                    .enumerate()
                    .map(|(index, color)| (color, index as u32 + 1))
                    .collect(),
                sidecar: None,
            })
        }
    }

    fn from_sidecar(colors: &BTreeSet<String>, sidecar: &Path) -> Result<Self> {
        let source = fs::read_to_string(sidecar)
            .with_context(|| format!("failed to read color sidecar {}", sidecar.display()))?;
        let parsed: ColorSidecar = toml::from_str(&source)
            .with_context(|| format!("failed to parse color sidecar {}", sidecar.display()))?;
        let mut mapping = BTreeMap::new();
        for (source_color, filament) in parsed.colors {
            let normalized = normalize_hex_color(&source_color)
                .with_context(|| format!("sidecar color {source_color:?} is not a hex color"))?;
            if mapping.insert(normalized.clone(), filament).is_some() {
                bail!("sidecar maps source color #{normalized} more than once");
            }
        }

        let mapped_colors: BTreeSet<_> = mapping.keys().cloned().collect();
        let missing: Vec<_> = colors.difference(&mapped_colors).cloned().collect();
        if !missing.is_empty() {
            bail!(
                "color sidecar is not exhaustive; missing {}",
                missing
                    .iter()
                    .map(|color| format!("#{color}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        let extra: Vec<_> = mapping
            .keys()
            .filter(|color| !colors.contains(*color))
            .cloned()
            .collect();
        if !extra.is_empty() {
            bail!(
                "color sidecar refers to absent colors: {}",
                extra
                    .iter()
                    .map(|color| format!("#{color}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        Ok(Self {
            source_to_filament: mapping,
            sidecar: Some(sidecar.to_owned()),
        })
    }

    /// Apply a label-local override. Exact hex keys win over resolved index keys.
    pub fn with_overrides(&self, overrides: &BTreeMap<String, u32>) -> Result<Self> {
        let existing_indices: BTreeSet<u32> = self.source_to_filament.values().copied().collect();
        let existing_colors: BTreeSet<_> = self.source_to_filament.keys().cloned().collect();
        let mut by_index = BTreeMap::new();
        let mut by_color = BTreeMap::new();

        for (key, target) in overrides {
            if let Some(color) = normalize_hex_color(key) {
                if !existing_colors.contains(&color) {
                    bail!("color override refers to absent source color #{color}");
                }
                by_color.insert(color, *target);
            } else {
                let index: u32 = key.parse().with_context(|| {
                    format!("color override key {key:?} must be a filament index or hex color")
                })?;
                if !existing_indices.contains(&index) {
                    bail!("color override refers to nonexistent resolved filament {index}");
                }
                by_index.insert(index, *target);
            }
        }

        let source_to_filament = self
            .source_to_filament
            .iter()
            .map(|(color, index)| {
                let target = by_color
                    .get(color)
                    .or_else(|| by_index.get(index))
                    .copied()
                    .unwrap_or(*index);
                (color.clone(), target)
            })
            .collect();

        Ok(Self {
            source_to_filament,
            sidecar: self.sidecar.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct PreviewPalette {
    by_filament: BTreeMap<u32, String>,
    by_color: BTreeMap<String, u32>,
}

impl PreviewPalette {
    pub fn new(filaments: impl IntoIterator<Item = u32>) -> Result<Self> {
        let filaments: BTreeSet<_> = filaments.into_iter().collect();
        if filaments.len() > 0x01_00_00_00 {
            bail!("a preview cannot represent more than 2^24 distinct filaments");
        }

        let mut by_filament = BTreeMap::new();
        let mut by_color = BTreeMap::new();
        for filament in filaments {
            let mut candidate = fallback_preview_rgb(filament);
            while by_color.contains_key(&format!("{candidate:06x}")) {
                // This odd step visits every 24-bit value before repeating.
                candidate = candidate.wrapping_add(0x9e_37_79) & 0x00ff_ffff;
            }
            let color = format!("{candidate:06x}");
            by_color.insert(color.clone(), filament);
            by_filament.insert(filament, color);
        }
        Ok(Self {
            by_filament,
            by_color,
        })
    }

    pub fn color(&self, filament: u32) -> &str {
        self.by_filament
            .get(&filament)
            .map(String::as_str)
            .expect("palette contains every rendered filament")
    }

    pub fn filament(&self, red: u8, green: u8, blue: u8) -> Option<u32> {
        self.by_color
            .get(&format!("{red:02x}{green:02x}{blue:02x}"))
            .copied()
    }
}

pub fn recolor_svg(
    source: &str,
    mapping: &BTreeMap<String, u32>,
    palette: &PreviewPalette,
) -> String {
    let mut result = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'#' {
            let available = bytes.len() - index - 1;
            let digit_count = if available >= 6
                && bytes[index + 1..index + 7]
                    .iter()
                    .all(u8::is_ascii_hexdigit)
            {
                6
            } else if available >= 3
                && bytes[index + 1..index + 4]
                    .iter()
                    .all(u8::is_ascii_hexdigit)
            {
                3
            } else {
                0
            };
            if digit_count > 0 {
                let token = &source[index + 1..index + digit_count + 1];
                if let Some(normalized) = normalize_hex_color(token)
                    && let Some(filament) = mapping.get(&normalized)
                {
                    result.push('#');
                    result.push_str(palette.color(*filament));
                    index += digit_count + 1;
                    continue;
                }
            }
        }
        let character = source[index..].chars().next().expect("valid UTF-8");
        result.push(character);
        index += character.len_utf8();
    }
    result
}

fn fallback_preview_rgb(filament: u32) -> u32 {
    const COLORS: [u32; 10] = [
        0xeaeaea, 0x43484d, 0xa7d293, 0x8aaed6, 0xe1927a, 0xf5d578, 0xa795d2, 0x89dad3, 0xeab97d,
        0x999487,
    ];
    // Give higher filament IDs stable, visually varied fallback colors for previews.
    COLORS
        .get(filament as usize)
        .copied()
        .unwrap_or_else(|| filament.wrapping_mul(2_654_435_761) & 0x00ff_ffff)
}

pub fn discover_colors(svg: &str) -> Result<BTreeSet<String>> {
    let document = Document::parse(svg).context("invalid SVG XML")?;
    let mut result = BTreeSet::new();
    for node in document.descendants().filter(|node| node.is_element()) {
        if !matches!(
            node.tag_name().name(),
            "path" | "rect" | "circle" | "ellipse" | "line" | "polygon" | "polyline" | "text"
        ) {
            continue;
        }

        // Fill and stroke are both converted to filled paths by usvg. Resolve
        // inherited paint here so every color that can reach the exporter is
        // assigned a filament before normalization.
        if node.tag_name().name() != "line" {
            collect_paint(&node, "fill", Some("#000000"), &mut result)?;
        }
        collect_paint(&node, "stroke", None, &mut result)?;
    }
    Ok(result)
}

fn collect_paint(
    node: &roxmltree::Node<'_, '_>,
    property: &str,
    default: Option<&str>,
    result: &mut BTreeSet<String>,
) -> Result<()> {
    let paint = node
        .ancestors()
        .filter(|ancestor| ancestor.is_element())
        .find_map(|ancestor| {
            let style = parse_style(ancestor.attribute("style").unwrap_or(""));
            style
                .get(property)
                .cloned()
                .or_else(|| ancestor.attribute(property).map(str::to_owned))
                .filter(|value| !value.trim().eq_ignore_ascii_case("inherit"))
        })
        .or_else(|| default.map(str::to_owned));
    let Some(paint) = paint else {
        return Ok(());
    };
    let paint = paint.trim();
    if paint.eq_ignore_ascii_case("none") {
        return Ok(());
    }
    let color = normalize_hex_color(paint).with_context(|| {
        format!("unsupported {property} {paint:?}; icon/template colors must use hex")
    })?;
    result.insert(color);
    Ok(())
}

fn parse_style(style: &str) -> BTreeMap<String, String> {
    style
        .split(';')
        .filter_map(|declaration| declaration.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_mapping_uses_lexical_order() {
        let colors = discover_colors(
            r##"<svg><path fill="#ff0000"/><path fill="#000000"/><path fill="#0000ff"/></svg>"##,
        )
        .unwrap();
        let mapping: BTreeMap<_, _> = colors
            .into_iter()
            .enumerate()
            .map(|(index, color)| (color, index as u32 + 1))
            .collect();
        assert_eq!(mapping["000000"], 1);
        assert_eq!(mapping["0000ff"], 2);
        assert_eq!(mapping["ff0000"], 3);
    }

    #[test]
    fn discovers_inherited_stroke_colors() {
        let colors = discover_colors(
            r##"<svg stroke="#123456"><path fill="none" d="M0 0L1 1"/><line x2="1"/></svg>"##,
        )
        .unwrap();
        assert_eq!(colors, BTreeSet::from(["123456".to_owned()]));
    }

    #[test]
    fn recolors_short_and_long_hex_values() {
        let mapping = BTreeMap::from([("ffffff".to_owned(), 3)]);
        let palette = PreviewPalette::new([3]).unwrap();
        assert_eq!(
            recolor_svg(
                r##"<path fill="#fff"/><path style="fill:#FFFFFF"/>"##,
                &mapping,
                &palette,
            ),
            r##"<path fill="#8aaed6"/><path style="fill:#8aaed6"/>"##
        );
    }

    #[test]
    fn palettes_are_reversible_and_resolve_collisions() {
        // Multiples of 2^24 have the same hashed fallback before collision resolution.
        let palette = PreviewPalette::new([0x01_00_00_00, 0x02_00_00_00]).unwrap();
        assert_eq!(palette.filament(0, 0, 0), Some(0x01_00_00_00));
        let second_color = palette.color(0x02_00_00_00);
        assert_ne!(second_color, "000000");
        let rgb = u32::from_str_radix(second_color, 16).unwrap();
        assert_eq!(
            palette.filament((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8),
            Some(0x02_00_00_00)
        );
    }

    #[test]
    fn exact_override_wins_over_index_override() {
        let base = ColorMapping {
            source_to_filament: BTreeMap::from([
                ("000000".into(), 0),
                ("ff0000".into(), 1),
                ("00ff00".into(), 1),
            ]),
            sidecar: None,
        };
        let mapped = base
            .with_overrides(&BTreeMap::from([("1".into(), 3), ("#ff0000".into(), 5)]))
            .unwrap();
        assert_eq!(mapped.source_to_filament["000000"], 0);
        assert_eq!(mapped.source_to_filament["00ff00"], 3);
        assert_eq!(mapped.source_to_filament["ff0000"], 5);
    }
}
