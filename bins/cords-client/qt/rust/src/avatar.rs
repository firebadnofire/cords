//! Local-only source staging and incremental crop/encoding. Never publishes.
use anyhow::{Context as _, Result, ensure};
use cords_avatar_codec::{ImageInfo, encoding::Encoder, playback::Decoder};
use cords_avatar_source::SourceDecoder;
use std::{fs::File, io::Read as _, path::Path, time::{Duration, Instant}};

pub(super) struct Draft {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub heads: Vec<String>,
    pub id: String,
}
struct Frame {
    pixels: Vec<u8>, width: u32, height: u32, bits: u32,
    numerator: u32, denominator: u32,
}
enum Source { Image(SourceDecoder), Jxl(Decoder) }
impl Source {
    fn open(bytes: &[u8]) -> Result<Self> {
        if bytes.starts_with(&[0xff, 0x0a]) || bytes.starts_with(b"\0\0\0\x0cJXL ") {
            Ok(Self::Jxl(Decoder::new(bytes)?))
        } else {
            let source = SourceDecoder::new(bytes.to_vec()).map_err(anyhow::Error::msg)?;
            ensure!(u64::from(source.width()) * u64::from(source.height()) * u64::from(source.bits() / 8) * 4 <= 64 * 1024 * 1024, "Picture exceeds the 64 MiB decoded-canvas limit");
            Ok(Self::Image(source))
        }
    }
    fn next(&mut self) -> Result<Option<Frame>> {
        match self {
            Self::Image(source) => Ok(source.next_frame().map_err(anyhow::Error::msg)?.map(|mut f| Frame {
                width: f.width(), height: f.height(), bits: u32::from(f.bits()),
                numerator: f.numerator(), denominator: f.denominator(), pixels: f.take_pixels(),
            })),
            Self::Jxl(source) => source.next_frame()?.map(|f| {
                let divisor = gcd(f.duration_ticks, f.ticks_numerator);
                Ok(Frame { width: f.width, height: f.height, bits: f.bits,
                    numerator: (f.duration_ticks / divisor).checked_mul(f.ticks_denominator).context("Animation duration overflow")?,
                    denominator: f.ticks_numerator / divisor, pixels: f.pixels })
            }).transpose(),
        }
    }
    fn loops(&self) -> u32 { match self { Self::Image(s) => s.loops(), Self::Jxl(s) => s.animation().1 } }
    fn animated(&self) -> bool { match self { Self::Image(s) => s.animated(), Self::Jxl(s) => s.animation().0 } }
}
fn check(deadline: Instant) -> Result<()> {
    ensure!(Instant::now() < deadline, "Avatar processing exceeded 120 seconds");
    Ok(())
}
fn metadata(width: u32, height: u32, bits: u32, numerator: u32, loops: u32) -> ImageInfo {
    ImageInfo { width, height, bits, frames: 0, loops, ticks_per_second_numerator: numerator, ticks_per_second_denominator: 1, durations: Vec::new() }
}
fn resize(frame: &Frame, crop: (u32, u32, u32, u32), width: u32, height: u32) -> Vec<u8> {
    let (x, y, crop_width, crop_height) = crop;
    let stride = (frame.bits / 8 * 4) as usize;
    let mut pixels = vec![0; width as usize * height as usize * stride];
    for row in 0..height {
        for column in 0..width {
            let sx = u64::from(x) + (u64::from(column) * 2 + 1) * u64::from(crop_width) / (u64::from(width) * 2);
            let sy = u64::from(y) + (u64::from(row) * 2 + 1) * u64::from(crop_height) / (u64::from(height) * 2);
            let from = (sy as usize * frame.width as usize + sx as usize) * stride;
            let to = (row as usize * width as usize + column as usize) * stride;
            pixels[to..to + stride].copy_from_slice(&frame.pixels[from..from + stride]);
        }
    }
    pixels
}
pub(super) fn prepare(path: &Path) -> Result<(Draft, Vec<u8>)> {
    let deadline = Instant::now() + Duration::from_secs(120);
    // Bound reads even if a selected file grows after its metadata was checked.
    let file = File::open(path).context("Cannot open selected picture")?;
    ensure!(file.metadata()?.is_file() && file.metadata()?.len() <= 25_000_000, "Choose a picture smaller than 25 MB");
    let mut bytes = Vec::new();
    file.take(25_000_001).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 25_000_000, "Picture exceeds 25 MB");
    let mut source = Source::open(&bytes)?;
    let frame = source.next()?.context("Picture contains no frames")?;
    check(deadline)?;
    let longest = frame.width.max(frame.height);
    let width = (u64::from(frame.width) * u64::from(longest.min(1024)) / u64::from(longest)).max(1) as u32;
    let height = (u64::from(frame.height) * u64::from(longest.min(1024)) / u64::from(longest)).max(1) as u32;
    let mut encoder = Encoder::new(&metadata(width, height, frame.bits, 1, 1), false, deadline)?;
    encoder.push(&resize(&frame, (0, 0, frame.width, frame.height), width, height), 0, true)?;
    let preview = encoder.finish()?;
    Ok((Draft { bytes, width: frame.width, height: frame.height, heads: Vec::new(), id: uuid::Uuid::now_v7().to_string() }, preview))
}
pub(super) fn crop(width: u32, height: u32, x: f64, y: f64, zoom: f64) -> Result<(u32, u32, u32)> {
    ensure!(width > 0 && height > 0 && x.is_finite() && y.is_finite() && zoom.is_finite() && (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y) && (1.0..=6.0).contains(&zoom), "Invalid crop coordinates");
    let side = (f64::from(width.min(height)) / zoom).floor().max(1.0) as u32;
    let left = (x * f64::from(width) - f64::from(side) / 2.0).clamp(0.0, f64::from(width - side)).floor() as u32;
    let top = (y * f64::from(height) - f64::from(side) / 2.0).clamp(0.0, f64::from(height - side)).floor() as u32;
    Ok((left, top, side))
}
fn gcd(mut a: u32, mut b: u32) -> u32 { while b != 0 { (a, b) = (b, a % b); } a }
pub(super) fn encode(draft: &Draft, x: f64, y: f64, zoom: f64) -> Result<Vec<u8>> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let (left, top, side) = crop(draft.width, draft.height, x, y, zoom)?;
    let output_side = side.min(1024);
    // Scan only rational durations; retain no animation frame collection. A second
    // incremental pass applies the same crop to every composited source frame.
    let mut source = Source::open(&draft.bytes)?;
    let loops = source.loops();
    let animated = source.animated();
    let mut durations = Vec::new();
    let mut timebase = 1_u32;
    let mut bits = 8;
    while let Some(frame) = source.next()? {
        check(deadline)?;
        ensure!(frame.width == draft.width && frame.height == draft.height, "Source geometry changed");
        ensure!(frame.denominator > 0 && durations.len() < 1000, "Invalid animation duration/count");
        bits = frame.bits;
        let divisor = frame.denominator / gcd(frame.numerator, frame.denominator);
        timebase = (timebase / gcd(timebase, divisor)).checked_mul(divisor).context("Animation timebase cannot be represented exactly")?;
        durations.push((frame.numerator, frame.denominator));
    }
    drop(source);
    ensure!(!durations.is_empty(), "Picture contains no frames");
    let mut encoder = Encoder::new(&metadata(output_side, output_side, bits, timebase, loops), animated, deadline)?;
    let mut source = Source::open(&draft.bytes)?;
    for (index, (numerator, denominator)) in durations.iter().enumerate() {
        check(deadline)?;
        let frame = source.next()?.context("Incomplete animation")?;
        let ticks = u32::try_from(u64::from(*numerator) * u64::from(timebase) / u64::from(*denominator))?;
        encoder.push(&resize(&frame, (left, top, side, side), output_side, output_side), ticks, index + 1 == durations.len())?;
    }
    drop(source);
    let output = encoder.finish()?;
    check(deadline)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crop_clamps_edges_and_rejects_invalid_input() -> Result<()> {
        assert_eq!(crop(800, 400, 0.5, 0.5, 1.0)?, (200, 0, 400));
        assert_eq!(crop(800, 400, 1.0, 0.0, 2.0)?, (600, 0, 200));
        assert!(crop(800, 400, f64::NAN, 0.5, 1.0).is_err());
        assert!(crop(800, 400, 0.5, 0.5, 0.0).is_err());
        Ok(())
    }
}
