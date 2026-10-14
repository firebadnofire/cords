//! APNG uses raw png frame controls so 16-bit samples and fractional delays are
//! never routed through `image::Frame`'s 8-bit animation representation.
use super::{DecodedFrame, MAX_ALLOC, MAX_FRAMES, MAX_METADATA, dimensions};
use image::metadata::Orientation;
use png::{BitDepth, BlendOp, ColorType, DisposeOp, FrameControl};
use std::io::Cursor;

pub(super) struct PngFrames {
    reader: png::Reader<Cursor<Vec<u8>>>,
    raw: Vec<u8>,
    canvas: Vec<u16>,
    previous: Vec<u16>,
    color: super::color::ColorTransform,
    last: Option<FrameControl>,
    remaining: u32,
    separate_default: bool,
    pub width: u32,
    pub height: u32,
    pub bits: u8,
    pub loops: u32,
    pub animated: bool,
    pub orientation: Orientation,
}

impl PngFrames {
    pub(super) fn new(input: Cursor<Vec<u8>>) -> Result<Self, String> {
        let input_len = input.get_ref().len();
        let mut decoder = png::Decoder::new_with_limits(
            input,
            png::Limits {
                bytes: MAX_METADATA,
            },
        );
        decoder.set_transformations(png::Transformations::EXPAND);
        // Source text is irrelevant to pixels. ICC and Exif remain available.
        decoder.set_ignore_text_chunk(true);
        let reader = decoder.read_info().map_err(|e| e.to_string())?;
        let info = reader.info();
        let (width, height) = (info.width, info.height);
        let pixels = dimensions(width, height)?;
        let (remaining, loops, animated) =
            info.animation_control.map_or((1, 1, false), |animation| {
                (animation.num_frames, animation.num_plays, true)
            });
        if remaining == 0 || remaining > MAX_FRAMES {
            return Err("PNG exceeds animation frame limit".into());
        }
        if info.icc_profile.as_deref().unwrap_or_default().len() > MAX_METADATA {
            return Err("PNG color profile exceeds 1 MiB".into());
        }
        let orientation = info
            .exif_metadata
            .as_deref()
            .and_then(Orientation::from_exif_chunk)
            .unwrap_or(Orientation::NoTransforms);
        let separate_default = animated && info.frame_control.is_none();
        let size = reader
            .output_buffer_size()
            .ok_or("PNG output allocation overflow")?;
        // Worst case: raw frame, 16-bit canvas, Previous disposal snapshot and
        // returned 16-bit RGBA bytes and color transform samples. WASM also has a
        // hard 128 MiB linear memory ceiling including codec/internal allocations.
        if pixels * 32 + size as u64 + input_len as u64 + MAX_METADATA as u64 > MAX_ALLOC {
            return Err("PNG working buffers exceed source-decoder memory budget".into());
        }
        let bits = if info.bit_depth == BitDepth::Sixteen {
            16
        } else {
            8
        };
        let color = super::color::png(info, bits)?;
        Ok(Self {
            reader,
            raw: vec![0; size],
            canvas: vec![0; usize::try_from(pixels).map_err(|_| "PNG canvas overflow")? * 4],
            previous: Vec::new(),
            color,
            last: None,
            remaining,
            separate_default,
            width,
            height,
            bits,
            loops,
            animated,
            orientation,
        })
    }

    fn dispose_previous(&mut self) -> Result<(), String> {
        if let Some(last) = self.last {
            match last.dispose_op {
                DisposeOp::None => {}
                DisposeOp::Previous => {
                    if self.previous.len() != self.canvas.len() {
                        return Err("Missing APNG disposal snapshot".into());
                    }
                    self.canvas.copy_from_slice(&self.previous);
                }
                DisposeOp::Background => {
                    for y in last.y_offset..last.y_offset + last.height {
                        let start = ((y * self.width + last.x_offset) * 4) as usize;
                        self.canvas[start..start + last.width as usize * 4].fill(0);
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn next(&mut self) -> Result<Option<DecodedFrame>, String> {
        if self.remaining == 0 {
            self.reader.finish().map_err(|e| e.to_string())?;
            return Ok(None);
        }
        if self.separate_default {
            // The default PNG is not an animation frame. Decode it safely, then
            // discard it without including it in the output timeline/canvas.
            self.reader
                .next_frame(&mut self.raw)
                .map_err(|e| e.to_string())?;
            self.separate_default = false;
        }
        self.dispose_previous()?;
        let output = self
            .reader
            .next_frame(&mut self.raw)
            .map_err(|e| e.to_string())?;
        let control = self.reader.info().frame_control.unwrap_or(FrameControl {
            width: self.width,
            height: self.height,
            delay_num: 0,
            delay_den: 1,
            ..FrameControl::default()
        });
        if control.width != output.width
            || control.height != output.height
            || control
                .x_offset
                .checked_add(control.width)
                .is_none_or(|end| end > self.width)
            || control
                .y_offset
                .checked_add(control.height)
                .is_none_or(|end| end > self.height)
        {
            return Err("Invalid PNG frame rectangle".into());
        }
        if control.dispose_op == DisposeOp::Previous {
            self.previous.resize(self.canvas.len(), 0);
            self.previous.copy_from_slice(&self.canvas);
        }
        let channels = output.color_type.samples();
        let sample_bytes = usize::from(self.bits / 8);
        let raw = &self.raw[..output.buffer_size()];
        let maximum = if self.bits == 16 { 65535 } else { 255 };
        for y in 0..output.height {
            for x in 0..output.width {
                let offset =
                    (y as usize * output.width as usize + x as usize) * channels * sample_bytes;
                let sample = |channel: usize| -> u16 {
                    let offset = offset + channel * sample_bytes;
                    if self.bits == 16 {
                        u16::from_be_bytes([raw[offset], raw[offset + 1]])
                    } else {
                        u16::from(raw[offset])
                    }
                };
                let source = match output.color_type {
                    ColorType::Grayscale => [sample(0), sample(0), sample(0), maximum],
                    ColorType::GrayscaleAlpha => [sample(0), sample(0), sample(0), sample(1)],
                    ColorType::Rgb => [sample(0), sample(1), sample(2), maximum],
                    ColorType::Rgba => [sample(0), sample(1), sample(2), sample(3)],
                    ColorType::Indexed => return Err("PNG palette expansion failed".into()),
                };
                let index =
                    (((y + control.y_offset) * self.width + x + control.x_offset) * 4) as usize;
                let destination = &mut self.canvas[index..index + 4];
                if control.blend_op == BlendOp::Source {
                    destination.copy_from_slice(&source);
                } else {
                    blend_over(destination, source, maximum)?;
                }
            }
        }
        self.last = Some(control);
        self.remaining -= 1;
        let mut pixels = Vec::with_capacity(self.canvas.len() * sample_bytes);
        for &sample in &self.canvas {
            if self.bits == 16 {
                pixels.extend_from_slice(&sample.to_le_bytes());
            } else {
                pixels.push(u8::try_from(sample).map_err(|_| "PNG sample overflow")?);
            }
        }
        self.color.apply(&mut pixels)?;
        Ok(Some(DecodedFrame {
            pixels,
            width: self.width,
            height: self.height,
            bits: self.bits,
            numerator: if self.animated {
                u32::from(control.delay_num)
            } else {
                0
            },
            // APNG defines denominator zero to mean 100, not zero seconds.
            denominator: if control.delay_den == 0 {
                100
            } else {
                u32::from(control.delay_den)
            },
        }))
    }
}

fn blend_over(destination: &mut [u16], source: [u16; 4], maximum: u16) -> Result<(), String> {
    let max = u64::from(maximum);
    let alpha = u64::from(source[3]);
    let destination_alpha = u64::from(destination[3]);
    let combined = alpha * max + destination_alpha * (max - alpha);
    if combined == 0 {
        destination.fill(0);
        return Ok(());
    }
    for channel in 0..3 {
        let weighted = u64::from(source[channel]) * alpha * max
            + u64::from(destination[channel]) * destination_alpha * (max - alpha);
        // Both expressions are bounded by maximum, including 16-bit input.
        destination[channel] = u16::try_from((weighted + combined / 2) / combined)
            .map_err(|_| "PNG blend sample overflow")?;
    }
    destination[3] =
        u16::try_from((combined + max / 2) / max).map_err(|_| "PNG blend alpha overflow")?;
    Ok(())
}
