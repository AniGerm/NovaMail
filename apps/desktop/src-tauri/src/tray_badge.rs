//! Paint a red unread-count badge onto the tray / taskbar icon.

use tauri::image::Image;

/// 3×5 digit bitmaps (MSB left). Space-efficient for tray overlays.
const DIGITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111], // 0
    [0b010, 0b110, 0b010, 0b010, 0b111], // 1
    [0b111, 0b001, 0b111, 0b100, 0b111], // 2
    [0b111, 0b001, 0b111, 0b001, 0b111], // 3
    [0b101, 0b101, 0b111, 0b001, 0b001], // 4
    [0b111, 0b100, 0b111, 0b001, 0b111], // 5
    [0b111, 0b100, 0b111, 0b101, 0b111], // 6
    [0b111, 0b001, 0b010, 0b010, 0b010], // 7
    [0b111, 0b101, 0b111, 0b101, 0b111], // 8
    [0b111, 0b101, 0b111, 0b001, 0b111], // 9
];

/// Build a tray icon with an optional red badge showing `count`.
/// `count == 0` returns the base icon unchanged (cloned RGBA).
pub fn icon_with_badge(base: &Image<'_>, count: u32) -> Image<'static> {
    let width = base.width();
    let height = base.height();
    let mut rgba = base.rgba().to_vec();
    if count == 0 || width < 16 || height < 16 {
        return Image::new_owned(rgba, width, height);
    }

    let label = if count > 99 {
        "99+".to_string()
    } else {
        count.to_string()
    };

    let scale = (width.min(height) / 32).max(1);
    let digit_w = 3 * scale;
    let digit_h = 5 * scale;
    let gap = scale;
    let pad_x = 2 * scale;
    let pad_y = 1 * scale;
    let chars: Vec<char> = label.chars().collect();
    let content_w = chars.len() as u32 * digit_w
        + chars.len().saturating_sub(1) as u32 * gap;
    let badge_w = (content_w + pad_x * 2).max(height / 3);
    let badge_h = digit_h + pad_y * 2;
    let badge_x = width.saturating_sub(badge_w);
    let badge_y = 0u32;

    fill_circle_rect(
        &mut rgba,
        width,
        height,
        badge_x,
        badge_y,
        badge_w,
        badge_h,
        [220, 38, 38, 255],
    );

    let mut cursor_x = badge_x + (badge_w.saturating_sub(content_w)) / 2;
    let cursor_y = badge_y + pad_y;
    for ch in chars {
        if ch == '+' {
            // tiny plus: horizontal + vertical bar
            fill_rect(
                &mut rgba,
                width,
                height,
                cursor_x,
                cursor_y + 2 * scale,
                digit_w,
                scale,
                [255, 255, 255, 255],
            );
            fill_rect(
                &mut rgba,
                width,
                height,
                cursor_x + scale,
                cursor_y + scale,
                scale,
                3 * scale,
                [255, 255, 255, 255],
            );
            cursor_x += digit_w + gap;
            continue;
        }
        if let Some(d) = ch.to_digit(10) {
            draw_digit(
                &mut rgba,
                width,
                height,
                cursor_x,
                cursor_y,
                scale,
                d as usize,
                [255, 255, 255, 255],
            );
            cursor_x += digit_w + gap;
        }
    }

    Image::new_owned(rgba, width, height)
}

fn fill_circle_rect(
    rgba: &mut [u8],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    color: [u8; 4],
) {
    // Rounded pill: ellipse covering the badge rect.
    let cx = x as f32 + w as f32 / 2.0;
    let cy = y as f32 + h as f32 / 2.0;
    let rx = w as f32 / 2.0;
    let ry = h as f32 / 2.0;
    for py in y..y.saturating_add(h).min(height) {
        for px in x..x.saturating_add(w).min(width) {
            let dx = (px as f32 + 0.5 - cx) / rx;
            let dy = (py as f32 + 0.5 - cy) / ry;
            if dx * dx + dy * dy <= 1.0 {
                put(rgba, width, px, py, color);
            }
        }
    }
}

fn fill_rect(
    rgba: &mut [u8],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    color: [u8; 4],
) {
    for py in y..y.saturating_add(h).min(height) {
        for px in x..x.saturating_add(w).min(width) {
            put(rgba, width, px, py, color);
        }
    }
}

fn draw_digit(
    rgba: &mut [u8],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    scale: u32,
    digit: usize,
    color: [u8; 4],
) {
    let bitmap = DIGITS[digit.min(9)];
    for (row, bits) in bitmap.iter().enumerate() {
        for col in 0..3u32 {
            if bits & (1 << (2 - col)) != 0 {
                fill_rect(
                    rgba,
                    width,
                    height,
                    x + col * scale,
                    y + row as u32 * scale,
                    scale,
                    scale,
                    color,
                );
            }
        }
    }
}

fn put(rgba: &mut [u8], width: u32, x: u32, y: u32, color: [u8; 4]) {
    let idx = ((y * width + x) * 4) as usize;
    if idx + 3 < rgba.len() {
        rgba[idx] = color[0];
        rgba[idx + 1] = color[1];
        rgba[idx + 2] = color[2];
        rgba[idx + 3] = color[3];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_changes_pixels_when_count_nonzero() {
        let w = 32u32;
        let h = 32u32;
        let base_pixels = vec![0u8; (w * h * 4) as usize];
        let base = Image::new_owned(base_pixels.clone(), w, h);
        let badged = icon_with_badge(&base, 3);
        let badged_rgba: &[u8] = badged.rgba();
        assert_ne!(badged_rgba, base_pixels.as_slice());
        let plain = icon_with_badge(&base, 0);
        let plain_rgba: &[u8] = plain.rgba();
        assert_eq!(plain_rgba, base_pixels.as_slice());
    }
}
