use anyhow::{Result, bail};

use crate::template::{IconAlignment, IconBox, IconDirection};

#[derive(Debug, Clone, PartialEq)]
pub enum RowItem {
    Icon { name: String, aspect_ratio: f64 },
    Spacer { size: f64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlacedIcon {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Fit ordered icons into a horizontal or vertical template box. Icons preserve
/// aspect ratio, spacers act along the flow axis, and no implicit gaps are added.
pub fn layout_icons(icon_box: &IconBox, items: &[RowItem]) -> Result<Vec<PlacedIcon>> {
    if items.is_empty() {
        return Ok(Vec::new());
    }
    if items.iter().any(|item| match item {
        RowItem::Icon { aspect_ratio, .. } => !aspect_ratio.is_finite() || *aspect_ratio <= 0.0,
        RowItem::Spacer { size } => !size.is_finite() || *size < 0.0,
    }) {
        bail!("icon ratios and spacer sizes must be finite and non-negative");
    }

    let fixed_size: f64 = items
        .iter()
        .filter_map(|item| match item {
            RowItem::Spacer { size } => Some(*size),
            RowItem::Icon { .. } => None,
        })
        .sum();
    let icon_count = items
        .iter()
        .filter(|item| matches!(item, RowItem::Icon { .. }))
        .count();
    if icon_count == 0 {
        bail!("icon box content must contain at least one icon");
    }

    match icon_box.direction {
        IconDirection::Horizontal => layout_horizontal(icon_box, items, fixed_size),
        IconDirection::Vertical => layout_vertical(icon_box, items, fixed_size),
    }
}

fn layout_horizontal(
    icon_box: &IconBox,
    items: &[RowItem],
    fixed_width: f64,
) -> Result<Vec<PlacedIcon>> {
    if fixed_width > icon_box.width {
        bail!(
            "icon spacers use {fixed_width} units but the box is only {} units wide",
            icon_box.width
        );
    }
    let aspect_sum: f64 = items
        .iter()
        .filter_map(|item| match item {
            RowItem::Icon { aspect_ratio, .. } => Some(*aspect_ratio),
            RowItem::Spacer { .. } => None,
        })
        .sum();
    let icon_height = icon_box
        .height
        .min((icon_box.width - fixed_width) / aspect_sum);
    if icon_height <= 0.0 {
        bail!("icons have no space left after fixed spacers");
    }
    let used_width = fixed_width + icon_height * aspect_sum;
    let mut cursor = icon_box.x + alignment_offset(icon_box.width, used_width, icon_box.alignment);
    let y = icon_box.y + (icon_box.height - icon_height) / 2.0;
    let mut result = Vec::new();

    for item in items {
        match item {
            RowItem::Spacer { size } => cursor += size,
            RowItem::Icon { name, aspect_ratio } => {
                let width = icon_height * aspect_ratio;
                result.push(PlacedIcon {
                    name: name.clone(),
                    x: cursor,
                    y,
                    width,
                    height: icon_height,
                });
                cursor += width;
            }
        }
    }
    Ok(result)
}

fn layout_vertical(
    icon_box: &IconBox,
    items: &[RowItem],
    fixed_height: f64,
) -> Result<Vec<PlacedIcon>> {
    if fixed_height > icon_box.height {
        bail!(
            "icon spacers use {fixed_height} units but the box is only {} units high",
            icon_box.height
        );
    }
    let inverse_aspect_sum: f64 = items
        .iter()
        .filter_map(|item| match item {
            RowItem::Icon { aspect_ratio, .. } => Some(1.0 / aspect_ratio),
            RowItem::Spacer { .. } => None,
        })
        .sum();
    let icon_width = icon_box
        .width
        .min((icon_box.height - fixed_height) / inverse_aspect_sum);
    if icon_width <= 0.0 {
        bail!("icons have no space left after fixed spacers");
    }
    let used_height = fixed_height + icon_width * inverse_aspect_sum;
    let mut cursor =
        icon_box.y + alignment_offset(icon_box.height, used_height, icon_box.alignment);
    let mut result = Vec::new();

    for item in items {
        match item {
            RowItem::Spacer { size } => cursor += size,
            RowItem::Icon { name, aspect_ratio } => {
                let height = icon_width / aspect_ratio;
                result.push(PlacedIcon {
                    name: name.clone(),
                    x: icon_box.x + (icon_box.width - icon_width) / 2.0,
                    y: cursor,
                    width: icon_width,
                    height,
                });
                cursor += height;
            }
        }
    }
    Ok(result)
}

fn alignment_offset(available: f64, used: f64, alignment: IconAlignment) -> f64 {
    match alignment {
        IconAlignment::Start => 0.0,
        IconAlignment::Center => (available - used) / 2.0,
        IconAlignment::End => available - used,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icon_box(direction: IconDirection, alignment: IconAlignment) -> IconBox {
        IconBox {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 10.0,
            direction,
            alignment,
        }
    }

    #[test]
    fn fits_horizontal_icons_without_implicit_gaps() {
        let result = layout_icons(
            &icon_box(IconDirection::Horizontal, IconAlignment::Start),
            &[
                RowItem::Icon {
                    name: "square".into(),
                    aspect_ratio: 1.0,
                },
                RowItem::Spacer { size: 2.0 },
                RowItem::Icon {
                    name: "wide".into(),
                    aspect_ratio: 2.0,
                },
            ],
        )
        .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].x, 0.0);
        assert_eq!(result[1].x, 8.0);
        assert_eq!(result[1].width, 12.0);
    }

    #[test]
    fn aligns_horizontal_rows_to_the_end() {
        let result = layout_icons(
            &icon_box(IconDirection::Horizontal, IconAlignment::End),
            &[RowItem::Icon {
                name: "square".into(),
                aspect_ratio: 1.0,
            }],
        )
        .unwrap();
        assert_eq!(result[0].x, 10.0);
        assert_eq!(result[0].width, 10.0);
    }

    #[test]
    fn lays_out_vertical_icons_top_down() {
        let mut box_info = icon_box(IconDirection::Vertical, IconAlignment::Start);
        box_info.width = 4.0;
        box_info.height = 12.0;
        let result = layout_icons(
            &box_info,
            &[
                RowItem::Icon {
                    name: "wide".into(),
                    aspect_ratio: 2.0,
                },
                RowItem::Spacer { size: 2.0 },
                RowItem::Icon {
                    name: "square".into(),
                    aspect_ratio: 1.0,
                },
            ],
        )
        .unwrap();
        assert_eq!(result[0].y, 0.0);
        assert_eq!(result[0].height, 2.0);
        assert_eq!(result[1].y, 4.0);
        assert_eq!(result[1].height, 4.0);
    }

    #[test]
    fn rejects_spacer_overflow() {
        let mut box_info = icon_box(IconDirection::Horizontal, IconAlignment::Center);
        box_info.width = 2.0;
        assert!(
            layout_icons(
                &box_info,
                &[
                    RowItem::Spacer { size: 3.0 },
                    RowItem::Icon {
                        name: "x".into(),
                        aspect_ratio: 1.0
                    }
                ]
            )
            .is_err()
        );
    }
}
