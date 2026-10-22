//! Pictures for the S75 Pro's 135 × 240 screen: decode a GIF or picture, fit each frame to the
//! screen, and pack it the way the keyboard takes it (`protocol::screen_image`).

use super::protocol::{rgb565, screen_image, SCREEN_H, SCREEN_MAX_FRAMES, SCREEN_W};
use crate::imagelight::Fit;
use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, ImageFormat, ImageReader, Rgba, RgbaImage};
use std::io::BufReader;
use std::path::Path;

/// Each frame of a picture with its length in ms; a still picture is one frame.
fn decode(path: &Path) -> Result<Vec<(RgbaImage, u32)>, String> {
    let bad = |e: image::ImageError| format!("Can't read the picture: {e}");
    let size = std::fs::metadata(path).map_err(|e| format!("Can't read the picture: {e}"))?.len();
    if size > crate::imagelight::MAX_BYTES {
        return Err("The picture is too big (at most 30 MB).".into());
    }
    let reader = ImageReader::open(path).map_err(|e| e.to_string())?.with_guessed_format().map_err(|e| e.to_string())?;
    if reader.format() == Some(ImageFormat::Gif) {
        let file = BufReader::new(std::fs::File::open(path).map_err(|e| e.to_string())?);
        let mut out = Vec::new();
        for f in GifDecoder::new(file).map_err(bad)?.into_frames().take(600) {
            let f = f.map_err(bad)?;
            let (n, d) = f.delay().numer_denom_ms();
            let ms = n / d.max(1);
            out.push((f.into_buffer(), if ms < 20 { 100 } else { ms }));
        }
        if !out.is_empty() {
            return Ok(out);
        }
    }
    Ok(vec![(reader.decode().map_err(bad)?.to_rgba8(), 1000)])
}

/// A frame laid onto the screen: covering it (Fill), inside it on black (Fit), or squeezed.
fn place(img: &RgbaImage, fit: Fit) -> RgbaImage {
    use image::imageops::{overlay, resize, FilterType};
    let (w, h) = (SCREEN_W as u32, SCREEN_H as u32);
    let (iw, ih) = (img.width().max(1) as f32, img.height().max(1) as f32);
    match fit {
        Fit::Stretch => resize(img, w, h, FilterType::Triangle),
        Fit::Fill | Fit::Fit => {
            let s = if fit == Fit::Fill { (w as f32 / iw).max(h as f32 / ih) } else { (w as f32 / iw).min(h as f32 / ih) };
            let (sw, sh) = (((iw * s).round() as u32).max(1), ((ih * s).round() as u32).max(1));
            let scaled = resize(img, sw, sh, FilterType::Triangle);
            let mut out = RgbaImage::from_pixel(w, h, Rgba([0, 0, 0, 255]));
            overlay(&mut out, &scaled, (w as i64 - sw as i64) / 2, (h as i64 - sh as i64) / 2);
            out
        }
    }
}

/// RGB565 pixels of a screen-sized frame (transparent parts black).
fn pixels(img: &RgbaImage) -> Vec<u8> {
    img.pixels()
        .flat_map(|p| {
            let a = p[3] as u16;
            let c = |v: u8| (v as u16 * a / 255) as u8;
            rgb565(c(p[0]), c(p[1]), c(p[2]))
        })
        .collect()
}

/// The frames fitted to the screen, at most 141: longer animations keep every n-th frame,
/// each held correspondingly longer.
pub fn frames(path: &Path, fit: Fit) -> Result<Vec<(RgbaImage, u32)>, String> {
    let all = decode(path)?;
    let step = all.len().div_ceil(SCREEN_MAX_FRAMES).max(1);
    Ok(all
        .chunks(step)
        .map(|group| (place(&group[0].0, fit), group.iter().map(|f| f.1).sum::<u32>().min(510)))
        .collect())
}

/// The image to upload: header and frames, padded to whole chunks.
pub fn image(path: &Path, fit: Fit) -> Result<(Vec<u8>, usize), String> {
    let f = frames(path, fit)?;
    let n = f.len();
    let packed: Vec<(Vec<u8>, u32)> = f.iter().map(|(img, ms)| (pixels(img), *ms)).collect();
    Ok((screen_image(&packed)?, n))
}

/// The first frame as a PNG, for the page's preview.
pub fn preview_png(path: &Path, fit: Fit) -> Result<Vec<u8>, String> {
    let f = frames(path, fit)?;
    let mut out = std::io::Cursor::new(Vec::new());
    f[0].0.write_to(&mut out, ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

/// Base64 for a data: URL.
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            s.push(if i <= c.len() { T[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_fit_the_screen() {
        let wide = RgbaImage::from_pixel(400, 100, Rgba([255, 0, 0, 255]));
        let fit = place(&wide, Fit::Fit);
        assert_eq!((fit.width(), fit.height()), (135, 240));
        assert_eq!(fit.get_pixel(67, 0).0, [0, 0, 0, 255]); // bars above and below
        assert_eq!(fit.get_pixel(67, 120).0, [255, 0, 0, 255]);
        let fill = place(&wide, Fit::Fill);
        assert_eq!(fill.get_pixel(0, 0).0, [255, 0, 0, 255]); // covered edge to edge
        assert_eq!(pixels(&fill).len(), 135 * 240 * 2);
    }

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
    }
}
