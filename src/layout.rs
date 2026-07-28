use anyhow::{Result, bail};

#[derive(Debug, Clone, PartialEq)]
pub enum RowItem {
    Icon { name: String, aspect_ratio: f64 },
    Spacer { width: f64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlacedIcon {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Fit an ordered row into a box. Icons share one height, preserve aspect ratio,
/// have no implicit gaps, and the complete row is centered.
pub fn layout_icon_row(
    box_x: f64,
    box_y: f64,
    box_width: f64,
    box_height: f64,
    items: &[RowItem],
) -> Result<Vec<PlacedIcon>> {
    let fixed_width: f64 = items
        .iter()
        .filter_map(|item| match item {
            RowItem::Spacer { width } => Some(*width),
            RowItem::Icon { .. } => None,
        })
        .sum();
    let aspect_sum: f64 = items
        .iter()
        .filter_map(|item| match item {
            RowItem::Icon { aspect_ratio, .. } => Some(*aspect_ratio),
            RowItem::Spacer { .. } => None,
        })
        .sum();

    if fixed_width > box_width {
        bail!("icon spacers use {fixed_width} units but the box is only {box_width} units wide");
    }
    if aspect_sum <= 0.0 {
        if fixed_width == 0.0 && items.is_empty() {
            return Ok(Vec::new());
        }
        bail!("icon row must contain at least one icon");
    }
    if items.iter().any(|item| match item {
        RowItem::Icon { aspect_ratio, .. } => !aspect_ratio.is_finite() || *aspect_ratio <= 0.0,
        RowItem::Spacer { width } => !width.is_finite() || *width < 0.0,
    }) {
        bail!("icon ratios and spacer widths must be finite and non-negative");
    }

    let icon_height = box_height.min((box_width - fixed_width) / aspect_sum);
    if icon_height <= 0.0 {
        bail!("icons have no space left after fixed spacers");
    }
    let used_width = fixed_width + icon_height * aspect_sum;
    let mut cursor = box_x + (box_width - used_width) / 2.0;
    let y = box_y + (box_height - icon_height) / 2.0;
    let mut result = Vec::new();

    for item in items {
        match item {
            RowItem::Spacer { width } => cursor += width,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_icons_without_implicit_gaps() {
        let result = layout_icon_row(
            0.0,
            0.0,
            10.0,
            4.0,
            &[
                RowItem::Icon {
                    name: "square".into(),
                    aspect_ratio: 1.0,
                },
                RowItem::Spacer { width: 2.0 },
                RowItem::Icon {
                    name: "wide".into(),
                    aspect_ratio: 2.0,
                },
            ],
        )
        .unwrap();
        assert_eq!(result.len(), 2);
        assert!((result[0].x - 0.0).abs() < 1e-9);
        assert!((result[0].width - 8.0 / 3.0).abs() < 1e-9);
        assert!((result[1].x - (8.0 / 3.0 + 2.0)).abs() < 1e-9);
        assert!((result[1].width - 16.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn centers_height_limited_rows() {
        let result = layout_icon_row(
            0.0,
            0.0,
            20.0,
            4.0,
            &[RowItem::Icon {
                name: "square".into(),
                aspect_ratio: 1.0,
            }],
        )
        .unwrap();
        assert_eq!(result[0].x, 8.0);
        assert_eq!(result[0].y, 0.0);
        assert_eq!(result[0].width, 4.0);
    }

    #[test]
    fn rejects_spacer_overflow() {
        assert!(
            layout_icon_row(
                0.0,
                0.0,
                2.0,
                2.0,
                &[
                    RowItem::Spacer { width: 3.0 },
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
