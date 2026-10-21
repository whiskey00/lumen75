//! Image animations: a GIF (or a still picture) played on the keys. Every frame is sampled
//! once, when the image loads: each key gets the average colour of the part of the picture
//! under it. Playing it is then only picking the frame for the time.

use crate::keymap::KEYS;
use crate::protocol::Rgb;
use image::codecs::gif::GifDecoder;
use image::codecs::png::PngDecoder;
use image::codecs::webp::WebPDecoder;
use image::{AnimationDecoder, ImageFormat, ImageReader, RgbaImage};
use serde::{Deserialize, Serialize};
use std::io::BufReader;
use std::path::Path;

/// Bigger files are refused: a keyboard shows about 16 x 6 keys, so they only cost time.
pub const MAX_BYTES: u64 = 30 * 1024 * 1024;
/// Frames beyond this are dropped (about a minute of a typical GIF).
pub const MAX_FRAMES: usize = 1000;

/// How the picture is laid over the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    /// Covers the whole board; what sticks out is cut off.
    #[default]
    Fill,
    /// The whole picture shows; the rest of the board stays dark.
    Fit,
    /// The picture is squeezed to the board's shape.
    Stretch,
}

/// A sampled image: each frame's length in seconds and every key's colour.
#[derive(Debug, Clone)]
pub struct Clip {
    frames: Vec<(f32, Vec<(u8, Rgb)>)>,
    total: f32,
}

impl Clip {
    pub fn frames(&self) -> usize {
        self.frames.len()
    }

    /// Length of one loop in seconds (a still picture counts as 1 s).
    pub fn seconds(&self) -> f32 {
        self.total
    }

    /// The key colours at `t` seconds, looping.
    pub fn at(&self, t: f32) -> &[(u8, Rgb)] {
        let mut left = t.rem_euclid(self.total);
        for (len, keys) in &self.frames {
            if left < *len {
                return keys;
            }
            left -= len;
        }
        &self.frames[self.frames.len() - 1].1
    }
}

/// A key's cell on the board, as fractions of the board (0..1), gaps included so the whole
/// picture is used.
struct Cell {
    index: u8,
    u: (f32, f32),
    v: (f32, f32),
}

fn cells() -> Vec<Cell> {
    let drawn: Vec<_> = KEYS.iter().filter(|k| k.rect[2] > 0).collect();
    let x0 = drawn.iter().map(|k| k.rect[0]).min().unwrap_or(0) as f32 - 6.0;
    let x1 = drawn.iter().map(|k| k.rect[2]).max().unwrap_or(1) as f32 + 6.0;
    let y0 = drawn.iter().map(|k| k.rect[1]).min().unwrap_or(0) as f32 - 5.0;
    let y1 = drawn.iter().map(|k| k.rect[3]).max().unwrap_or(1) as f32 + 5.0;
    let (w, h) = (x1 - x0, y1 - y0);
    drawn
        .iter()
        .map(|k| Cell {
            index: k.index,
            u: ((k.rect[0] as f32 - 6.0 - x0) / w, (k.rect[2] as f32 + 6.0 - x0) / w),
            v: ((k.rect[1] as f32 - 5.0 - y0) / h, (k.rect[3] as f32 + 5.0 - y0) / h),
        })
        .collect()
}

/// Width / height of the board.
fn board_aspect() -> f32 {
    let drawn: Vec<_> = KEYS.iter().filter(|k| k.rect[2] > 0).collect();
    let w = drawn.iter().map(|k| k.rect[2]).max().unwrap_or(1) - drawn.iter().map(|k| k.rect[0]).min().unwrap_or(0) + 12;
    let h = drawn.iter().map(|k| k.rect[3]).max().unwrap_or(1) - drawn.iter().map(|k| k.rect[1]).min().unwrap_or(0) + 10;
    w as f32 / h as f32
}

/// Every key's colour in one picture: the average of the pixels under its cell. Transparent
/// pixels and (for Fit) the area outside the picture count as black.
fn sample(img: &RgbaImage, fit: Fit, cells: &[Cell], aspect: f32) -> Vec<(u8, Rgb)> {
    let (w, h) = (img.width() as f32, img.height() as f32);
    // the part of the picture (in pixels, maybe beyond its edges) that maps onto the board
    let (cw, ch) = match fit {
        Fit::Fill => (w.min(h * aspect), h.min(w / aspect)),
        Fit::Fit => (w.max(h * aspect), h.max(w / aspect)),
        Fit::Stretch => (w, h),
    };
    let (ox, oy) = ((w - cw) / 2.0, (h - ch) / 2.0);
    cells
        .iter()
        .map(|c| {
            let span = |a: f32, b: f32, o: f32, len: f32| {
                let lo = (o + a * len).floor() as i64;
                let hi = ((o + b * len).ceil() as i64).max(lo + 1);
                (lo, hi)
            };
            let (xa, xb) = span(c.u.0, c.u.1, ox, cw);
            let (ya, yb) = span(c.v.0, c.v.1, oy, ch);
            let mut sum = [0u64; 3];
            for y in ya.max(0)..yb.min(img.height() as i64) {
                for x in xa.max(0)..xb.min(img.width() as i64) {
                    let p = img.get_pixel(x as u32, y as u32).0;
                    for i in 0..3 {
                        sum[i] += p[i] as u64 * p[3] as u64 / 255;
                    }
                }
            }
            let area = ((xb - xa) * (yb - ya)).max(1) as u64;
            (c.index, sum.map(|s| (s / area).min(255) as u8))
        })
        .collect()
}

/// GIFs often say 0 or 10 ms per frame and mean "as fast as the browser plays it": 100 ms.
fn frame_seconds(numer: u32, denom: u32) -> f32 {
    let ms = numer as f32 / denom.max(1) as f32;
    if ms < 20.0 { 0.1 } else { ms / 1000.0 }
}

/// Small enough to sample quickly, big enough that no key covers less than a few pixels.
fn shrink(img: RgbaImage) -> RgbaImage {
    const MAX: u32 = 256;
    if img.width() <= MAX && img.height() <= MAX {
        return img;
    }
    let s = MAX as f32 / img.width().max(img.height()) as f32;
    let (w, h) = (((img.width() as f32 * s) as u32).max(1), ((img.height() as f32 * s) as u32).max(1));
    image::imageops::resize(&img, w, h, image::imageops::FilterType::Triangle)
}

/// Decode and sample a picture: animated GIF, WebP or PNG, or any still PNG, JPEG, GIF or WebP.
pub fn load(path: &Path, fit: Fit) -> Result<Clip, String> {
    let size = std::fs::metadata(path).map_err(|e| format!("Can't read the image: {e}"))?.len();
    if size > MAX_BYTES {
        return Err(format!("The image is too big ({} MB; at most {} MB).", size / 1_048_576, MAX_BYTES / 1_048_576));
    }
    let bad = |e: image::ImageError| format!("Can't read the image: {e}");
    let reader = ImageReader::open(path).map_err(|e| format!("Can't read the image: {e}"))?.with_guessed_format().map_err(|e| format!("Can't read the image: {e}"))?;
    let format = reader.format();
    let open = || std::fs::File::open(path).map(BufReader::new).map_err(|e| format!("Can't read the image: {e}"));
    let (cells, aspect) = (cells(), board_aspect());
    let mut frames = Vec::new();
    let mut add_animation = |decoded: image::Frames| -> Result<(), String> {
        for f in decoded.take(MAX_FRAMES) {
            let f = f.map_err(bad)?;
            let (n, d) = f.delay().numer_denom_ms();
            let img = shrink(f.into_buffer());
            frames.push((frame_seconds(n, d), sample(&img, fit, &cells, aspect)));
        }
        Ok(())
    };
    match format {
        Some(ImageFormat::Gif) => add_animation(GifDecoder::new(open()?).map_err(bad)?.into_frames())?,
        Some(ImageFormat::WebP) => {
            let d = WebPDecoder::new(open()?).map_err(bad)?;
            if d.has_animation() {
                add_animation(d.into_frames())?;
            }
        }
        Some(ImageFormat::Png) => {
            let d = PngDecoder::new(open()?).map_err(bad)?;
            if d.is_apng().map_err(bad)? {
                add_animation(d.apng().map_err(bad)?.into_frames())?;
            }
        }
        Some(ImageFormat::Jpeg) => {}
        _ => return Err("That isn't a GIF, PNG, JPEG or WebP image.".into()),
    }
    if frames.is_empty() {
        // a still picture
        let img = shrink(reader.decode().map_err(bad)?.to_rgba8());
        frames.push((1.0, sample(&img, fit, &cells, aspect)));
    }
    let total = frames.iter().map(|f| f.0).sum::<f32>().max(0.02);
    Ok(Clip { frames, total })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Delay, Frame, Rgba};

    fn key(name: &str) -> u8 {
        crate::keymap::key_by_name(name).unwrap().index
    }

    fn colour(keys: &[(u8, Rgb)], k: u8) -> Rgb {
        keys.iter().find(|(i, _)| *i == k).unwrap().1
    }

    /// Left half red, right half blue.
    fn halves(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, _| if x < w / 2 { Rgba([255, 0, 0, 255]) } else { Rgba([0, 0, 255, 255]) })
    }

    #[test]
    fn keys_take_the_colour_under_them() {
        let keys = sample(&halves(320, 120), Fit::Stretch, &cells(), board_aspect());
        assert_eq!(keys.len(), 80);
        assert_eq!(colour(&keys, key("esc")), [255, 0, 0]);
        assert_eq!(colour(&keys, key("a")), [255, 0, 0]);
        assert_eq!(colour(&keys, key("pgdn")), [0, 0, 255]);
        assert_eq!(colour(&keys, key("right")), [0, 0, 255]);
    }

    #[test]
    fn fit_leaves_the_board_dark_around_a_tall_picture() {
        // a square picture on a wide board: Fit shows it in the middle, Fill covers everything
        let square = RgbaImage::from_pixel(100, 100, Rgba([0, 255, 0, 255]));
        let fit = sample(&square, Fit::Fit, &cells(), board_aspect());
        assert_eq!(colour(&fit, key("esc")), [0, 0, 0]);
        assert_eq!(colour(&fit, key("t")), [0, 255, 0]);
        let fill = sample(&square, Fit::Fill, &cells(), board_aspect());
        assert!(fill.iter().all(|(_, c)| *c == [0, 255, 0]));
    }

    #[test]
    fn transparent_pixels_are_dark() {
        let clear = RgbaImage::from_pixel(64, 24, Rgba([255, 255, 255, 0]));
        assert!(sample(&clear, Fit::Stretch, &cells(), board_aspect()).iter().all(|(_, c)| *c == [0, 0, 0]));
    }

    #[test]
    fn a_tiny_picture_still_lights_every_key() {
        let keys = sample(&halves(2, 1), Fit::Stretch, &cells(), board_aspect());
        assert!(keys.iter().all(|(_, c)| *c != [0, 0, 0]));
    }

    #[test]
    fn gif_frames_play_with_their_own_timing() {
        let dir = std::env::temp_dir().join(format!("lumen75-gif-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("two.gif");
        {
            let file = std::fs::File::create(&path).unwrap();
            let mut enc = image::codecs::gif::GifEncoder::new(file);
            let red = RgbaImage::from_pixel(32, 12, Rgba([255, 0, 0, 255]));
            let blue = RgbaImage::from_pixel(32, 12, Rgba([0, 0, 255, 255]));
            enc.encode_frame(Frame::from_parts(red, 0, 0, Delay::from_numer_denom_ms(200, 1))).unwrap();
            enc.encode_frame(Frame::from_parts(blue, 0, 0, Delay::from_numer_denom_ms(300, 1))).unwrap();
        }
        let clip = load(&path, Fit::Fill).unwrap();
        assert_eq!(clip.frames(), 2);
        assert!((clip.seconds() - 0.5).abs() < 0.001);
        let q = key("q");
        assert_eq!(colour(clip.at(0.1), q), [255, 0, 0]);
        assert_eq!(colour(clip.at(0.3), q), [0, 0, 255]);
        assert_eq!(colour(clip.at(0.6), q), [255, 0, 0]); // loops
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn zero_delays_play_at_ten_frames_a_second() {
        assert_eq!(frame_seconds(0, 1), 0.1);
        assert_eq!(frame_seconds(10, 1), 0.1);
        assert_eq!(frame_seconds(50, 1), 0.05);
    }
}
