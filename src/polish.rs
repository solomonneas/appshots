use std::path::Path;

use image::DynamicImage;
use image::GenericImageView;
use image::ImageBuffer;
use image::Rgba;
use image::RgbaImage;
use image::imageops;
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::backends::FrameExtents;
use crate::contract::PresentationStyleInfo;
use crate::util::AppError;

const AMBIENT_SHADOW_ALPHA: u8 = 52;
const KEY_SHADOW_ALPHA: u8 = 96;
const RIM_STRENGTH: f32 = 0.24;
const STREAK_STRENGTH: f32 = 0.09;
const GRAIN_STRENGTH: f32 = 2.4;
const REFERENCE_SIZE: f32 = 900.0;
const SHADOW_DOWNSCALE: u32 = 4;

type Palette = (&'static str, [[u8; 3]; 3], [u8; 3], [u8; 3]);

const PALETTES: [Palette; 5] = [
    (
        "violet-haze",
        [[49, 29, 130], [112, 52, 224], [44, 84, 228]],
        [235, 96, 190],
        [132, 196, 255],
    ),
    (
        "ember-glow",
        [[224, 158, 64], [214, 98, 40], [128, 46, 24]],
        [252, 214, 140],
        [170, 50, 30],
    ),
    (
        "aurora-teal",
        [[10, 72, 92], [18, 152, 128], [88, 206, 196]],
        [150, 235, 190],
        [36, 96, 176],
    ),
    (
        "rose-noir",
        [[40, 26, 50], [150, 38, 94], [236, 96, 122]],
        [250, 152, 100],
        [124, 72, 200],
    ),
    (
        "midnight-sky",
        [[14, 24, 58], [40, 78, 198], [92, 170, 248]],
        [142, 102, 248],
        [132, 228, 250],
    ),
];

#[derive(Debug, Clone)]
pub struct PresentationStyle {
    pub seed: u64,
    pub palette_name: String,
    pub stops: [[u8; 3]; 3],
    pub glow_a: [u8; 3],
    pub glow_b: [u8; 3],
    /// Base values tuned for a `REFERENCE_SIZE` capture; scaled at render time.
    pub padding: u32,
    pub corner_radius: u32,
    pub shadow_blur: f32,
    pub shadow_offset_y: i32,
    pub gradient_angle: f32,
    pub streak_angle: f32,
    pub streak_phase: f32,
    pub glow_a_pos: (f32, f32),
    pub glow_b_pos: (f32, f32),
}

pub fn random_style() -> PresentationStyle {
    let seed = rand::rng().random();
    style_from_seed(seed)
}

pub fn style_from_seed(seed: u64) -> PresentationStyle {
    let mut rng = StdRng::seed_from_u64(seed);
    let (name, stops, glow_a, glow_b) = PALETTES[rng.random_range(0..PALETTES.len())];
    PresentationStyle {
        seed,
        palette_name: name.to_string(),
        stops,
        glow_a,
        glow_b,
        padding: rng.random_range(100..=132),
        corner_radius: rng.random_range(18..=26),
        shadow_blur: rng.random_range(22.0..=34.0),
        shadow_offset_y: rng.random_range(16..=26),
        gradient_angle: rng.random_range(0.35..=1.15),
        streak_angle: rng.random_range(0.5..=1.05),
        streak_phase: rng.random_range(0.0..=std::f32::consts::TAU),
        glow_a_pos: (rng.random_range(0.55..=0.95), rng.random_range(0.0..=0.22)),
        glow_b_pos: (rng.random_range(0.05..=0.45), rng.random_range(0.72..=1.0)),
    }
}

pub fn render_codex_card(
    input_path: &Path,
    output_path: &Path,
    frame_extents: Option<FrameExtents>,
    style: &PresentationStyle,
) -> Result<(), AppError> {
    let mut input = image::open(input_path).map_err(|source| AppError::Image {
        path: input_path.to_path_buf(),
        source,
    })?;
    if let Some(extents) = frame_extents {
        input = crop_frame_extents(input, extents);
    }
    let canvas = compose_card(&input, style);
    canvas.save(output_path).map_err(|source| AppError::Image {
        path: output_path.to_path_buf(),
        source,
    })?;
    Ok(())
}

/// Pure composition step: rounded window on a styled backdrop with layered
/// shadows, rim highlight, and rounded canvas corners.
pub fn compose_card(input: &DynamicImage, style: &PresentationStyle) -> RgbaImage {
    let (window_width, window_height) = input.dimensions();
    let scale = (window_width.min(window_height) as f32 / REFERENCE_SIZE).clamp(0.7, 3.0);
    let padding = (style.padding as f32 * scale).round() as u32;
    let card_radius = ((style.corner_radius as f32 * scale).round() as u32).max(6);
    let outer_radius = (padding as f32 * 0.45).round() as u32;
    let rim_width = (1.6 * scale).clamp(1.0, 4.0);

    let window = rounded_window(input, card_radius, rim_width);
    let canvas_width = window_width + padding * 2;
    let canvas_height = window_height + padding * 2;
    let mut canvas = backdrop(canvas_width, canvas_height, style);

    let key_offset = (style.shadow_offset_y as f32 * scale).round() as i32;
    let ambient_offset = (style.shadow_offset_y as f32 * 1.8 * scale).round() as i32;
    let key_sigma = style.shadow_blur * scale;
    let ambient_sigma = style.shadow_blur * 2.4 * scale;

    let ambient = soft_shadow_layer(
        canvas_width,
        canvas_height,
        padding as i32,
        padding as i32 + ambient_offset,
        window_width,
        window_height,
        card_radius,
        ambient_sigma,
        AMBIENT_SHADOW_ALPHA,
    );
    alpha_composite(&mut canvas, &ambient, 0, 0);
    let key = soft_shadow_layer(
        canvas_width,
        canvas_height,
        padding as i32,
        padding as i32 + key_offset,
        window_width,
        window_height,
        card_radius,
        key_sigma,
        KEY_SHADOW_ALPHA,
    );
    alpha_composite(&mut canvas, &key, 0, 0);
    alpha_composite(&mut canvas, &window, padding as i32, padding as i32);
    round_canvas_corners(&mut canvas, outer_radius);
    canvas
}

impl PresentationStyle {
    pub fn info(&self) -> PresentationStyleInfo {
        PresentationStyleInfo {
            seed: self.seed,
            palette: self.palette_name.clone(),
            padding: self.padding,
            corner_radius: self.corner_radius,
            shadow_blur: self.shadow_blur,
            shadow_offset_y: self.shadow_offset_y,
        }
    }
}

fn crop_frame_extents(input: DynamicImage, extents: FrameExtents) -> DynamicImage {
    let (width, height) = input.dimensions();
    let horizontal = extents.left.saturating_add(extents.right);
    let vertical = extents.top.saturating_add(extents.bottom);
    if horizontal >= width || vertical >= height {
        return input;
    }
    input.crop_imm(
        extents.left,
        extents.top,
        width - horizontal,
        height - vertical,
    )
}

fn rounded_window(input: &DynamicImage, radius: u32, rim_width: f32) -> RgbaImage {
    let mut image = input.to_rgba8();
    let (width, height) = image.dimensions();
    for y in 0..height {
        for x in 0..width {
            let distance = inner_distance(x, y, width, height, radius);
            let coverage = (distance + 0.5).clamp(0.0, 1.0);
            let pixel = image.get_pixel_mut(x, y);
            if coverage < 1.0 {
                pixel.0[3] = ((f32::from(pixel.0[3]) * coverage).round()) as u8;
            }
            // Subtle rim highlight just inside the card edge.
            if distance > -0.5 && distance < rim_width + 1.0 {
                let band = if distance <= rim_width {
                    coverage
                } else {
                    (rim_width + 1.0 - distance).clamp(0.0, 1.0) * coverage
                };
                let amount = RIM_STRENGTH * band;
                for channel in 0..3 {
                    let value = f32::from(pixel.0[channel]);
                    pixel.0[channel] = (value + (255.0 - value) * amount).round() as u8;
                }
            }
        }
    }
    image
}

/// Signed distance to the inside of a rounded rectangle covering the full
/// `width` x `height` area. Positive inside, negative outside.
fn inner_distance(x: u32, y: u32, width: u32, height: u32, radius: u32) -> f32 {
    let half_width = width as f32 / 2.0;
    let half_height = height as f32 / 2.0;
    let radius = (radius as f32).min(half_width).min(half_height);
    let px = (x as f32 + 0.5 - half_width).abs();
    let py = (y as f32 + 0.5 - half_height).abs();
    let qx = px - (half_width - radius);
    let qy = py - (half_height - radius);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - radius;
    -outside
}

fn rounded_alpha(x: u32, y: u32, width: u32, height: u32, radius: u32) -> u8 {
    let coverage = (inner_distance(x, y, width, height, radius) + 0.5).clamp(0.0, 1.0);
    (coverage * 255.0).round() as u8
}

#[allow(clippy::too_many_arguments)]
fn soft_shadow_layer(
    canvas_width: u32,
    canvas_height: u32,
    rect_x: i32,
    rect_y: i32,
    rect_width: u32,
    rect_height: u32,
    radius: u32,
    sigma: f32,
    alpha: u8,
) -> RgbaImage {
    // Blur at reduced resolution, then upscale: visually identical for a
    // soft shadow and far cheaper than a full-resolution gaussian pass.
    let small_width = canvas_width.div_ceil(SHADOW_DOWNSCALE).max(1);
    let small_height = canvas_height.div_ceil(SHADOW_DOWNSCALE).max(1);
    let mut mask = RgbaImage::from_pixel(small_width, small_height, Rgba([0, 0, 0, 0]));
    for y in 0..small_height {
        for x in 0..small_width {
            let full_x = (x * SHADOW_DOWNSCALE) as i32 - rect_x;
            let full_y = (y * SHADOW_DOWNSCALE) as i32 - rect_y;
            if full_x < 0
                || full_y < 0
                || full_x >= rect_width as i32
                || full_y >= rect_height as i32
            {
                continue;
            }
            let coverage = rounded_alpha(
                full_x as u32,
                full_y as u32,
                rect_width,
                rect_height,
                radius,
            );
            if coverage == 0 {
                continue;
            }
            let shadow_alpha = ((u16::from(coverage) * u16::from(alpha)) / 255) as u8;
            mask.put_pixel(x, y, Rgba([0, 0, 0, shadow_alpha]));
        }
    }
    let blurred = imageops::blur(&mask, (sigma / SHADOW_DOWNSCALE as f32).max(0.5));
    imageops::resize(
        &blurred,
        canvas_width,
        canvas_height,
        imageops::FilterType::Triangle,
    )
}

fn round_canvas_corners(canvas: &mut RgbaImage, radius: u32) {
    if radius == 0 {
        return;
    }
    let (width, height) = canvas.dimensions();
    for y in 0..height {
        for x in 0..width {
            if x >= radius && x < width - radius && y >= radius && y < height - radius {
                continue;
            }
            let coverage = rounded_alpha(x, y, width, height, radius);
            if coverage < 255 {
                let pixel = canvas.get_pixel_mut(x, y);
                pixel.0[3] = ((u16::from(pixel.0[3]) * u16::from(coverage)) / 255) as u8;
            }
        }
    }
}

fn backdrop(width: u32, height: u32, style: &PresentationStyle) -> RgbaImage {
    let stops = style.stops.map(to_f32);
    let glow_a = to_f32(style.glow_a);
    let glow_b = to_f32(style.glow_b);
    let (gradient_cos, gradient_sin) = (style.gradient_angle.cos(), style.gradient_angle.sin());
    let gradient_norm = (gradient_cos + gradient_sin).max(f32::EPSILON);
    let (streak_cos, streak_sin) = (style.streak_angle.cos(), style.streak_angle.sin());
    ImageBuffer::from_fn(width, height, |x, y| {
        let fx = x as f32 / width.max(1) as f32;
        let fy = y as f32 / height.max(1) as f32;
        let t = ((fx * gradient_cos + fy * gradient_sin) / gradient_norm).clamp(0.0, 1.0);
        let mut color = if t < 0.5 {
            mix3(stops[0], stops[1], smoothstep(t * 2.0))
        } else {
            mix3(stops[1], stops[2], smoothstep((t - 0.5) * 2.0))
        };

        let glow_a_distance =
            ((fx - style.glow_a_pos.0).powi(2) + (fy - style.glow_a_pos.1).powi(2)).sqrt();
        color = mix3(
            color,
            glow_a,
            (1.0 - glow_a_distance / 0.85).clamp(0.0, 1.0).powi(2) * 0.55,
        );
        let glow_b_distance =
            ((fx - style.glow_b_pos.0).powi(2) + (fy - style.glow_b_pos.1).powi(2)).sqrt();
        color = mix3(
            color,
            glow_b,
            (1.0 - glow_b_distance / 0.9).clamp(0.0, 1.0).powi(2) * 0.48,
        );

        // Broad diagonal light streaks, like soft window light.
        let band = fx * streak_cos + fy * streak_sin;
        let streak = (band * 17.0 + style.streak_phase).sin() * 0.62
            + (band * 29.0 + style.streak_phase * 1.7).sin() * 0.38;
        if streak > 0.0 {
            color = mix3(color, [255.0, 255.0, 255.0], streak * STREAK_STRENGTH);
        } else {
            let dim = 1.0 + streak * 0.05;
            color = [color[0] * dim, color[1] * dim, color[2] * dim];
        }

        // Fine grain breaks up gradient banding.
        let grain = grain_noise(x, y, style.seed) * GRAIN_STRENGTH;
        Rgba([
            quantize(color[0] + grain),
            quantize(color[1] + grain),
            quantize(color[2] + grain),
            255,
        ])
    })
}

fn grain_noise(x: u32, y: u32, seed: u64) -> f32 {
    let mut hash = x
        .wrapping_mul(0x9E37_79B1)
        .wrapping_add(y.wrapping_mul(0x85EB_CA77))
        .wrapping_add(seed as u32);
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x7FEB_352D);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x846C_A68B);
    hash ^= hash >> 16;
    (hash as f32 / u32::MAX as f32) * 2.0 - 1.0
}

fn to_f32(color: [u8; 3]) -> [f32; 3] {
    [
        f32::from(color[0]),
        f32::from(color[1]),
        f32::from(color[2]),
    ]
}

fn mix3(start: [f32; 3], end: [f32; 3], amount: f32) -> [f32; 3] {
    let amount = amount.clamp(0.0, 1.0);
    [
        start[0] + (end[0] - start[0]) * amount,
        start[1] + (end[1] - start[1]) * amount,
        start[2] + (end[2] - start[2]) * amount,
    ]
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn quantize(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

fn alpha_composite(base: &mut RgbaImage, overlay: &RgbaImage, offset_x: i32, offset_y: i32) {
    let (base_width, base_height) = base.dimensions();
    for y in 0..overlay.height() {
        for x in 0..overlay.width() {
            let target_x = offset_x + x as i32;
            let target_y = offset_y + y as i32;
            if target_x < 0 || target_y < 0 {
                continue;
            }
            let target_x = target_x as u32;
            let target_y = target_y as u32;
            if target_x >= base_width || target_y >= base_height {
                continue;
            }
            let src = overlay.get_pixel(x, y);
            let alpha = f32::from(src.0[3]) / 255.0;
            if alpha == 0.0 {
                continue;
            }
            let dst = base.get_pixel(target_x, target_y);
            let inv_alpha = 1.0 - alpha;
            let out = Rgba([
                (f32::from(src.0[0]) * alpha + f32::from(dst.0[0]) * inv_alpha).round() as u8,
                (f32::from(src.0[1]) * alpha + f32::from(dst.0[1]) * inv_alpha).round() as u8,
                (f32::from(src.0[2]) * alpha + f32::from(dst.0[2]) * inv_alpha).round() as u8,
                255,
            ]);
            base.put_pixel(target_x, target_y, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_input(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            width,
            height,
            Rgba([200, 200, 200, 255]),
        ))
    }

    #[test]
    fn canvas_corners_are_transparent() {
        let canvas = compose_card(&test_input(400, 300), &style_from_seed(7));
        let (width, height) = canvas.dimensions();
        assert_eq!(canvas.get_pixel(0, 0).0[3], 0);
        assert_eq!(canvas.get_pixel(width - 1, 0).0[3], 0);
        assert_eq!(canvas.get_pixel(0, height - 1).0[3], 0);
        assert_eq!(canvas.get_pixel(width - 1, height - 1).0[3], 0);
        assert_eq!(canvas.get_pixel(width / 2, height / 2).0[3], 255);
    }

    #[test]
    fn canvas_adds_scaled_padding() {
        let style = style_from_seed(7);
        let canvas = compose_card(&test_input(400, 300), &style);
        let scale = (300.0_f32 / REFERENCE_SIZE).clamp(0.7, 3.0);
        let padding = (style.padding as f32 * scale).round() as u32;
        assert_eq!(canvas.dimensions(), (400 + padding * 2, 300 + padding * 2));
    }

    #[test]
    fn backdrop_varies_across_canvas() {
        let style = style_from_seed(11);
        let canvas = backdrop(320, 240, &style);
        let a = canvas.get_pixel(8, 8);
        let b = canvas.get_pixel(311, 231);
        assert_ne!(a.0[..3], b.0[..3]);
    }

    #[test]
    fn same_seed_renders_identically() {
        let input = test_input(200, 160);
        let first = compose_card(&input, &style_from_seed(42));
        let second = compose_card(&input, &style_from_seed(42));
        assert_eq!(first.as_raw(), second.as_raw());
    }

    #[test]
    fn every_seed_palette_is_known() {
        for seed in 0..32 {
            let style = style_from_seed(seed);
            assert!(
                PALETTES
                    .iter()
                    .any(|(name, _, _, _)| *name == style.palette_name)
            );
        }
    }

    #[test]
    fn inner_distance_sign_matches_geometry() {
        // Center of a 100x100 rect is deep inside.
        assert!(inner_distance(50, 50, 100, 100, 20) > 30.0);
        // The exact corner pixel is outside the rounded corner.
        assert!(inner_distance(0, 0, 100, 100, 20) < 0.0);
        // Edge midpoints sit on the border.
        assert!(inner_distance(50, 0, 100, 100, 20) < 1.0);
    }
}
