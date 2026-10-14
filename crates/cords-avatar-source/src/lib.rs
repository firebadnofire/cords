//! Incremental local avatar source decoding. No network, filesystem, identity,
//! or profile publication. The WASM host must impose aggregate memory/deadline
//! budgets, including this module's linear memory and copied frame buffers.
use image::{AnimationDecoder as _, DynamicImage, ImageDecoder as _, metadata::Orientation};
use std::io::Cursor;
use wasm_bindgen::prelude::*;

mod color;
mod png_frames;
mod source_metadata;
#[cfg(test)]
mod tests;

const MAX_INPUT: usize = 50_000_000;
const MAX_ALLOC: u64 = 128 * 1024 * 1024;
const MAX_METADATA: usize = 1024 * 1024;
const MAX_FRAMES: u32 = 1000;
const MAX_PIXELS: u64 = 100_000_000;

fn dimensions(width: u32, height: u32) -> Result<u64, String> {
    let pixels = u64::from(width) * u64::from(height);
    if width == 0 || height == 0 || width > 8192 || height > 8192 || pixels > 32_000_000 {
        return Err("Source image exceeds dimension/canvas limits".into());
    }
    Ok(pixels)
}

fn limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(MAX_ALLOC);
    limits
}

enum Frames {
    Png(Box<png_frames::PngFrames>),
    Animation(image::Frames<'static>),
    Still(Option<DynamicImage>),
}

/// One decoder owns its encoded input and bounded compositing state. Frames are
/// returned individually; callers must release each frame before requesting more.
#[wasm_bindgen]
pub struct SourceDecoder {
    frames: Frames,
    width: u32,
    height: u32,
    bits: u8,
    loops: u32,
    animated: bool,
    orientation: Orientation,
    icc: Vec<u8>,
    color: color::ColorTransform,
    count: u32,
    cumulative_pixels: u64,
    expected_gif_frames: Option<Vec<u16>>,
    failed: bool,
}

impl std::fmt::Debug for SourceDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceDecoder")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("count", &self.count)
            .field("failed", &self.failed)
            .finish_non_exhaustive()
    }
}

/// Straight-alpha RGBA; 16-bit samples are emitted in little-endian order.
#[wasm_bindgen]
#[derive(Debug)]
pub struct DecodedFrame {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    bits: u8,
    numerator: u32,
    denominator: u32,
}

#[wasm_bindgen]
impl DecodedFrame {
    /// Apply the same integer crop/output geometry to each composited frame.
    /// Nearest-neighbor sampling preserves integer samples and transparent RGB;
    /// it introduces no color-space or alpha blending during geometric sampling.
    /// # Errors
    /// Rejects invalid rectangles, upscaling, or output larger than 1024 pixels.
    pub fn crop_resize(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        output_width: u32,
        output_height: u32,
    ) -> Result<(), String> {
        if width == 0
            || height == 0
            || output_width == 0
            || output_height == 0
            || output_width > width
            || output_height > height
            || output_width > 1024
            || output_height > 1024
            || x.checked_add(width).is_none_or(|end| end > self.width)
            || y.checked_add(height).is_none_or(|end| end > self.height)
        {
            return Err("Invalid avatar crop or output dimensions".into());
        }
        let stride = usize::from(self.bits / 8) * 4;
        let length = usize::try_from(u64::from(output_width) * u64::from(output_height))
            .map_err(|_| "Avatar output size overflow")?
            * stride;
        let source_length = u64::from(self.width) * u64::from(self.height) * stride as u64;
        if self.pixels.len() as u64 != source_length {
            return Err("Avatar frame pixels are unavailable".into());
        }
        let mut output = vec![0; length];
        for row in 0..output_height {
            let source_y = u64::from(y)
                + (u64::from(row) * 2 + 1) * u64::from(height) / (u64::from(output_height) * 2);
            for column in 0..output_width {
                let source_x = u64::from(x)
                    + (u64::from(column) * 2 + 1) * u64::from(width)
                        / (u64::from(output_width) * 2);
                let source = usize::try_from(source_y * u64::from(self.width) + source_x)
                    .map_err(|_| "Avatar crop offset overflow")?
                    * stride;
                let target =
                    usize::try_from(u64::from(row) * u64::from(output_width) + u64::from(column))
                        .map_err(|_| "Avatar output offset overflow")?
                        * stride;
                output[target..target + stride]
                    .copy_from_slice(&self.pixels[source..source + stride]);
            }
        }
        self.pixels = output;
        self.width = output_width;
        self.height = output_height;
        Ok(())
    }

    /// Transfer/copy one frame; this consumes its internal pixel allocation.
    pub fn take_pixels(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.pixels)
    }
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.width
    }
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.height
    }
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn bits(&self) -> u8 {
        self.bits
    }
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn numerator(&self) -> u32 {
        self.numerator
    }
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn denominator(&self) -> u32 {
        self.denominator
    }
}

#[wasm_bindgen]
impl SourceDecoder {
    /// Open a supported source locally. Errors preserve the caller's saved avatar.
    /// # Errors
    /// Rejects unknown formats, malformed metadata and resource-limit violations.
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: Vec<u8>) -> Result<SourceDecoder, String> {
        if bytes.is_empty() || bytes.len() > MAX_INPUT {
            return Err("Source image exceeds 50,000,000 bytes".into());
        }
        let format = image::guess_format(&bytes).map_err(|e| e.to_string())?;
        let mut gif_metadata = None;
        if format == image::ImageFormat::Gif {
            gif_metadata = Some(source_metadata::gif(&bytes)?);
        }
        match format {
            image::ImageFormat::Png => source_metadata::png(&bytes)?,
            image::ImageFormat::WebP => source_metadata::webp(&bytes)?,
            image::ImageFormat::Jpeg => source_metadata::jpeg(&bytes)?,
            _ => {}
        }
        let mut source = Self {
            frames: Frames::Still(None),
            width: 0,
            height: 0,
            bits: 8,
            loops: 1,
            animated: false,
            orientation: Orientation::NoTransforms,
            icc: Vec::new(),
            color: color::ColorTransform::None,
            count: 0,
            cumulative_pixels: 0,
            expected_gif_frames: None,
            failed: false,
        };
        let reader = Cursor::new(bytes);
        match format {
            image::ImageFormat::Png => {
                let frames = png_frames::PngFrames::new(reader)?;
                source.width = frames.width;
                source.height = frames.height;
                source.bits = frames.bits;
                source.loops = frames.loops;
                source.animated = frames.animated;
                source.orientation = frames.orientation;
                source.frames = Frames::Png(Box::new(frames));
            }
            image::ImageFormat::Gif => {
                let mut decoder =
                    image::codecs::gif::GifDecoder::new(reader).map_err(|e| e.to_string())?;
                decoder.set_limits(limits()).map_err(|e| e.to_string())?;
                (source.width, source.height) = decoder.dimensions();
                dimensions(source.width, source.height)?;
                let metadata = gif_metadata.ok_or("Missing GIF preflight")?;
                source.animated = true; // Even one GIF frame can carry timing and loops.
                source.loops = metadata.loops;
                source.expected_gif_frames = Some(metadata.delays);
                source.frames = Frames::Animation(decoder.into_frames());
            }
            image::ImageFormat::WebP => {
                let mut decoder =
                    image::codecs::webp::WebPDecoder::new(reader).map_err(|e| e.to_string())?;
                decoder.set_limits(limits()).map_err(|e| e.to_string())?;
                (source.width, source.height) = decoder.dimensions();
                dimensions(source.width, source.height)?;
                source.orientation = decoder.orientation().map_err(|e| e.to_string())?;
                source.icc = decoder
                    .icc_profile()
                    .map_err(|e| e.to_string())?
                    .unwrap_or_default();
                source.animated = decoder.has_animation();
                source.loops = match decoder.loop_count() {
                    image::metadata::LoopCount::Infinite => 0,
                    image::metadata::LoopCount::Finite(count) => count.get(),
                };
                source.frames = if source.animated {
                    Frames::Animation(decoder.into_frames())
                } else {
                    Frames::Still(Some(
                        DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?,
                    ))
                };
            }
            image::ImageFormat::Jpeg => {
                let mut decoder =
                    image::codecs::jpeg::JpegDecoder::new(reader).map_err(|e| e.to_string())?;
                decoder.set_limits(limits()).map_err(|e| e.to_string())?;
                (source.width, source.height) = decoder.dimensions();
                dimensions(source.width, source.height)?;
                source.orientation = decoder.orientation().map_err(|e| e.to_string())?;
                source.icc = decoder
                    .icc_profile()
                    .map_err(|e| e.to_string())?
                    .unwrap_or_default();
                source.frames = Frames::Still(Some(
                    DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?,
                ));
            }
            _ => return Err("Unsupported avatar source format".into()),
        }
        source.finish_open()
    }

    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.width
    }
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.height
    }
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn bits(&self) -> u8 {
        self.bits
    }
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn loops(&self) -> u32 {
        self.loops
    }
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn animated(&self) -> bool {
        self.animated
    }
    /// Decode one composited frame, retaining no history of previous output frames.
    /// # Errors
    /// Malformed source data and excess work invalidate the entire decoder.
    pub fn next_frame(&mut self) -> Result<Option<DecodedFrame>, String> {
        if self.failed {
            return Err("Source decoder failed; dispose the operation".into());
        }
        let result = self.read_frame();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
}

impl SourceDecoder {
    fn finish_open(mut self) -> Result<Self, String> {
        if self.icc.len() > MAX_METADATA {
            return Err("Image color profile exceeds metadata limit".into());
        }
        self.color = color::ColorTransform::new(&self.icc, self.bits)?;
        self.icc = Vec::new();
        if matches!(
            self.orientation,
            Orientation::Rotate90
                | Orientation::Rotate270
                | Orientation::Rotate90FlipH
                | Orientation::Rotate270FlipH
        ) {
            std::mem::swap(&mut self.width, &mut self.height);
        }
        Ok(self)
    }

    fn read_frame(&mut self) -> Result<Option<DecodedFrame>, String> {
        let mut frame = match &mut self.frames {
            Frames::Png(png) => match png.next()? {
                Some(frame) => frame,
                None => return Ok(None),
            },
            Frames::Animation(frames) => {
                let Some(frame) = frames.next() else {
                    if self
                        .expected_gif_frames
                        .as_ref()
                        .is_some_and(|f| f.len() != self.count as usize)
                    {
                        return Err("GIF frame count differs from source metadata".into());
                    }
                    return Ok(None);
                };
                let frame = frame.map_err(|e| e.to_string())?;
                let (numerator, denominator) = frame.delay().numer_denom_ms();
                let buffer = frame.into_buffer();
                DecodedFrame {
                    width: buffer.width(),
                    height: buffer.height(),
                    bits: 8,
                    pixels: buffer.into_raw(),
                    numerator,
                    denominator: denominator
                        .checked_mul(1000)
                        .ok_or("Source timing overflow")?,
                }
            }
            Frames::Still(image) => {
                let Some(image) = image.take() else {
                    return Ok(None);
                };
                let buffer = image.into_rgba8();
                DecodedFrame {
                    width: buffer.width(),
                    height: buffer.height(),
                    bits: 8,
                    pixels: buffer.into_raw(),
                    numerator: 0,
                    denominator: 1,
                }
            }
        };
        if let Some(delays) = &self.expected_gif_frames {
            // image's millisecond delay is exact; GIF's finite repeat extension
            // counts repeats after the first play, unlike JXL's total loops.
            let delay = *delays
                .get(self.count as usize)
                .ok_or("Unexpected GIF frame")?;
            if u64::from(frame.numerator) * 100 != u64::from(delay) * u64::from(frame.denominator) {
                return Err("GIF timing differs from source metadata".into());
            }
            frame.numerator = u32::from(delay);
            frame.denominator = 100;
        }
        let pixels = dimensions(frame.width, frame.height)?;
        if self.count >= MAX_FRAMES || self.cumulative_pixels > MAX_PIXELS - pixels {
            return Err("Animation exceeds frame/cumulative decoded-pixel limits".into());
        }
        self.count += 1;
        self.cumulative_pixels += pixels;
        if self.orientation != Orientation::NoTransforms {
            let mut image = if frame.bits == 16 {
                let samples = frame
                    .pixels
                    .chunks_exact(2)
                    .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
                    .collect();
                let buffer = image::ImageBuffer::<image::Rgba<u16>, _>::from_raw(
                    frame.width,
                    frame.height,
                    samples,
                )
                .ok_or("Invalid 16-bit RGBA frame")?;
                DynamicImage::ImageRgba16(buffer)
            } else {
                let buffer = image::RgbaImage::from_raw(frame.width, frame.height, frame.pixels)
                    .ok_or("Invalid RGBA frame")?;
                DynamicImage::ImageRgba8(buffer)
            };
            image.apply_orientation(self.orientation);
            frame.width = image.width();
            frame.height = image.height();
            frame.pixels = if frame.bits == 16 {
                image
                    .into_rgba16()
                    .into_raw()
                    .iter()
                    .flat_map(|sample| sample.to_le_bytes())
                    .collect()
            } else {
                image.into_rgba8().into_raw()
            };
        }
        self.color.apply(&mut frame.pixels)?;
        Ok(Some(frame))
    }
}
