use crate::ffi;
use anyhow::{Context as _, Result, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use cords_avatar_codec::playback::Decoder;

#[derive(Debug)]
pub struct NativeImage(Decoder);

/// Open only an embedded JXL payload; this never fetches a URL or reads a path.
/// # Errors
/// Returns a bounded input or pinned decoder error.
pub fn open_image(source: &str) -> Result<Box<NativeImage>> {
    ensure!(
        source.len() <= 6_666_700,
        "Embedded avatar exceeds input limit"
    );
    let payload = source
        .strip_prefix("data:image/jxl;base64,")
        .context("Expected an embedded JPEG XL image")?;
    let bytes = STANDARD
        .decode(payload)
        .context("Invalid embedded JPEG XL encoding")?;
    Ok(Box::new(NativeImage(Decoder::new(&bytes)?)))
}
impl NativeImage {
    /// # Errors
    /// Returns a malformed image, end-of-stream or bounded decoder error.
    pub fn next_frame(&mut self) -> Result<ffi::NativeFrame> {
        let frame = self
            .0
            .next_frame()?
            .context("JPEG XL animation is complete")?;
        Ok(ffi::NativeFrame {
            width: frame.width,
            height: frame.height,
            bits: frame.bits,
            duration_ticks: frame.duration_ticks,
            ticks_numerator: frame.ticks_numerator,
            ticks_denominator: frame.ticks_denominator,
            loops: frame.loops,
            last: frame.last,
            pixels: frame.pixels,
        })
    }
}
