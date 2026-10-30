// Cover art: decoding, the soft backdrop copy and the accent colours.
use slint::{Color, Image, Rgba8Pixel, SharedPixelBuffer};

pub struct Art {
    pub image: Image,
    pub backdrop: Image,
    pub colors: Option<(Color, Color)>,
}

/// Decodes an image (PNG, JPEG, WebP) from bytes or a data: URL.
pub fn decode(bytes: &[u8]) -> Option<Art> {
    let img = image::load_from_memory(bytes).ok()?;
    // Covers are shown at most ~220 px; keep memory small.
    let img = if img.width() > 400 || img.height() > 400 {
        img.resize(400, 400, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    let image = to_slint(&rgba);
    let tiny = image::imageops::resize(&rgba, 24, 24, image::imageops::FilterType::Triangle);
    let backdrop = to_slint(&saturate(tiny, 1.5));
    Some(Art { image, backdrop, colors: colors(&rgba) })
}

pub fn decode_data_url(url: &str) -> Option<Art> {
    use base64::Engine;
    let b64 = url.split_once(";base64,")?.1;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    decode(&bytes)
}

fn to_slint(rgba: &image::RgbaImage) -> Image {
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(rgba.as_raw(), rgba.width(), rgba.height()))
}

fn saturate(mut img: image::RgbaImage, k: f32) -> image::RgbaImage {
    for p in img.pixels_mut() {
        let [r, g, b, _] = p.0;
        let l = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
        let f = |c: u8| (l + (c as f32 - l) * k).clamp(0.0, 255.0) as u8;
        p.0 = [f(r), f(g), f(b), 255];
    }
    img
}

/// Two accent colours: the most vivid bright colour and a contrasting one,
/// lifted so they read on a dark background.
fn colors(rgba: &image::RgbaImage) -> Option<(Color, Color)> {
    let small = image::imageops::resize(rgba, 24, 24, image::imageops::FilterType::Triangle);
    #[derive(Default, Clone, Copy)]
    struct B { r: f32, g: f32, b: f32, n: f32, w: f32 }
    let mut buckets: std::collections::HashMap<(u8, u8, u8), B> = Default::default();
    for p in small.pixels() {
        let [r, g, b, _] = p.0;
        let max = r.max(g).max(b) as f32;
        let min = r.min(g).min(b) as f32;
        if max < 40.0 {
            continue;
        }
        let sat = (max - min) / max;
        let e = buckets.entry((r >> 5, g >> 5, b >> 5)).or_default();
        e.r += r as f32;
        e.g += g as f32;
        e.b += b as f32;
        e.n += 1.0;
        e.w += (0.15 + sat * 2.0) * (max / 255.0);
    }
    let mut v: Vec<B> = buckets.into_values().collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| b.w.total_cmp(&a.w));
    let lift = |e: &B| {
        let (mut r, mut g, mut b) = (e.r / e.n, e.g / e.n, e.b / e.n);
        let max = r.max(g).max(b);
        if max < 215.0 {
            let k = 215.0 / max;
            r *= k;
            g *= k;
            b *= k;
        }
        Color::from_rgb_u8(r.min(255.0) as u8, g.min(255.0) as u8, b.min(255.0) as u8)
    };
    let first = v[0];
    let second = v
        .iter()
        .skip(1)
        .find(|e| (e.r / e.n - first.r / first.n).abs() + (e.b / e.n - first.b / first.n).abs() > 60.0)
        .or(v.get(1))
        .copied()
        .unwrap_or(first);
    Some((lift(&first), lift(&second)))
}

/// The progress part of the cover ring as SVG path commands in the 56×56
/// viewbox used by the overlay (a rounded square, clockwise from the top-left).
pub fn ring_path(p: f32) -> String {
    let p = p.clamp(0.0, 1.0);
    if p <= 0.001 {
        return String::new();
    }
    let (x0, y0, x1, y1, r) = (1.5f32, 1.5f32, 54.5f32, 54.5f32, 7.0f32);
    let side = (x1 - x0) - 2.0 * r;
    let arc = std::f32::consts::FRAC_PI_2 * r;
    let total = 4.0 * side + 4.0 * arc;
    let mut left = p * total;
    let mut s = format!("M {} {}", x0 + r, y0);
    // Straight edges and corners, in order.
    let segs: [(f32, f32, f32, f32, Option<(f32, f32, f32)>); 8] = [
        (x0 + r, y0, x1 - r, y0, None),
        (0.0, 0.0, 0.0, 0.0, Some((x1 - r, y0 + r, -90.0))),
        (x1, y0 + r, x1, y1 - r, None),
        (0.0, 0.0, 0.0, 0.0, Some((x1 - r, y1 - r, 0.0))),
        (x1 - r, y1, x0 + r, y1, None),
        (0.0, 0.0, 0.0, 0.0, Some((x0 + r, y1 - r, 90.0))),
        (x0, y1 - r, x0, y0 + r, None),
        (0.0, 0.0, 0.0, 0.0, Some((x0 + r, y0 + r, 180.0))),
    ];
    for (ax, ay, bx, by, corner) in segs {
        if left <= 0.0 {
            break;
        }
        match corner {
            None => {
                let f = (left / side).min(1.0);
                s += &format!(" L {:.2} {:.2}", ax + (bx - ax) * f, ay + (by - ay) * f);
                left -= side;
            }
            Some((cx, cy, start)) => {
                let f = (left / arc).min(1.0);
                let a = (start + 90.0 * f).to_radians();
                s += &format!(" A {r} {r} 0 0 1 {:.2} {:.2}", cx + r * a.cos(), cy + r * a.sin());
                left -= arc;
            }
        }
    }
    s
}

pub fn fmt_time(sec: f64) -> String {
    let s = sec.max(0.0).floor() as u64;
    let (h, m, ss) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{ss:02}")
    } else {
        format!("{m}:{ss:02}")
    }
}
