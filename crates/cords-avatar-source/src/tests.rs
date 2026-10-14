// Fixture construction is intentionally fatal: a failed encoder invalidates the test.
#![allow(clippy::unwrap_used)]
use super::*;

fn apng(bits: png::BitDepth, separate_default: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(bits);
        encoder.set_animated(4, 3).unwrap();
        encoder.set_sep_def_img(separate_default).unwrap();
        let mut writer = encoder.write_header().unwrap();
        let sample = |pixel: &[u16]| -> Vec<u8> {
            pixel
                .iter()
                .flat_map(|sample| {
                    if bits == png::BitDepth::Sixteen {
                        sample.to_be_bytes().to_vec()
                    } else {
                        vec![u8::try_from(*sample).unwrap()]
                    }
                })
                .collect()
        };
        let max = if bits == png::BitDepth::Sixteen {
            65535
        } else {
            255
        };
        if separate_default {
            writer
                .write_image_data(&sample(&[max, max, max, max, max, max, max, max]))
                .unwrap();
        }
        writer.set_frame_delay(0, 100).unwrap();
        writer
            .write_image_data(&sample(&[max, 0, 0, max, 0, 0, max, max]))
            .unwrap();
        writer.set_frame_dimension(1, 1).unwrap();
        writer.set_frame_position(0, 0).unwrap();
        writer.set_blend_op(png::BlendOp::Over).unwrap();
        writer.set_dispose_op(png::DisposeOp::Previous).unwrap();
        writer.set_frame_delay(1, 60).unwrap();
        writer
            .write_image_data(&sample(&[0, max, 0, max / 2 + 1]))
            .unwrap();
        writer.set_frame_position(1, 0).unwrap();
        writer.set_blend_op(png::BlendOp::Source).unwrap();
        writer.set_dispose_op(png::DisposeOp::Background).unwrap();
        writer.set_frame_delay(1, 7).unwrap();
        writer.write_image_data(&sample(&[0, max, 0, max])).unwrap();
        writer.set_frame_position(0, 0).unwrap();
        writer.set_dispose_op(png::DisposeOp::None).unwrap();
        writer.set_frame_delay(0, 0).unwrap();
        writer.write_image_data(&sample(&[max, 0, 0, max])).unwrap();
        writer.finish().unwrap();
    }
    bytes
}

fn gif(repeat: Option<gif::Repeat>) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut bytes, 2, 1, &[]).unwrap();
        if let Some(repeat) = repeat {
            encoder.set_repeat(repeat).unwrap();
        }
        let mut first_pixels = [255, 0, 0, 255, 0, 0, 255, 255];
        let first = gif::Frame::from_rgba(2, 1, &mut first_pixels);
        encoder.write_frame(&first).unwrap();
        let mut second_pixels = [0, 255, 0, 255];
        let mut second = gif::Frame::from_rgba(1, 1, &mut second_pixels);
        second.delay = 1;
        second.dispose = gif::DisposalMethod::Previous;
        encoder.write_frame(&second).unwrap();
        let mut third_pixels = [0, 0, 0, 0];
        let mut third = gif::Frame::from_rgba(1, 1, &mut third_pixels);
        third.left = 1;
        third.delay = 3;
        encoder.write_frame(&third).unwrap();
    }
    bytes
}

#[test]
fn gif_timing_repeat_semantics_and_disposal_are_preserved() {
    for (repeat, loops) in [
        (None, 1),
        (Some(gif::Repeat::Infinite), 0),
        (Some(gif::Repeat::Finite(2)), 3),
    ] {
        let mut decoder = SourceDecoder::new(gif(repeat)).unwrap();
        assert_eq!(decoder.loops(), loops);
        assert!(decoder.animated());
        for (index, duration) in [0, 1, 3].into_iter().enumerate() {
            let frame = decoder.next_frame().unwrap().unwrap();
            assert_eq!((frame.numerator(), frame.denominator()), (duration, 100));
            assert_eq!(
                &frame.pixels[..4],
                if index == 1 {
                    &[0, 255, 0, 255]
                } else {
                    &[255, 0, 0, 255]
                }
            );
            assert_eq!(&frame.pixels[4..], &[0, 0, 255, 255]);
        }
        assert!(decoder.next_frame().unwrap().is_none());
    }
}

#[test]
fn apng_preserves_zero_and_fractional_delays_blending_disposal_and_default_image() {
    for separate_default in [false, true] {
        let mut decoder = SourceDecoder::new(apng(png::BitDepth::Eight, separate_default)).unwrap();
        assert_eq!(decoder.loops(), 3);
        let mut frames = Vec::new();
        while let Some(frame) = decoder.next_frame().unwrap() {
            frames.push(frame);
        }
        assert_eq!(frames.len(), 4);
        assert_eq!(
            frames
                .iter()
                .map(|f| (f.numerator, f.denominator))
                .collect::<Vec<_>>(),
            [(0, 100), (1, 60), (1, 7), (0, 100)]
        );
        assert_eq!(frames[0].pixels, [255, 0, 0, 255, 0, 0, 255, 255]);
        assert_eq!(frames[1].pixels, [127, 128, 0, 255, 0, 0, 255, 255]);
        assert_eq!(frames[2].pixels, [255, 0, 0, 255, 0, 255, 0, 255]);
        assert_eq!(frames[3].pixels, [255, 0, 0, 255, 0, 0, 0, 0]);
    }
}

#[test]
fn apng_16_bit_compositing_does_not_reduce_precision() {
    let mut decoder = SourceDecoder::new(apng(png::BitDepth::Sixteen, false)).unwrap();
    assert_eq!(decoder.bits(), 16);
    let first = decoder.next_frame().unwrap().unwrap();
    assert_eq!(first.bits, 16);
    let second = decoder.next_frame().unwrap().unwrap();
    let samples: Vec<_> = second
        .pixels
        .chunks_exact(2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .collect();
    assert_eq!(samples, [32767, 32768, 0, 65535, 0, 0, 65535, 65535]);
}

#[test]
fn corrupted_input_fails_without_returning_a_static_fallback() {
    assert!(SourceDecoder::new(b"not an image".to_vec()).is_err());
    let mut bytes = apng(png::BitDepth::Eight, false);
    // Corrupt the first IDAT's compressed data (the CRC must then fail).
    let idat = bytes.windows(4).position(|w| w == b"IDAT").unwrap();
    bytes[idat + 4] ^= 0x80;
    let mut decoder = SourceDecoder::new(bytes).unwrap();
    assert!(decoder.next_frame().is_err());
    assert!(decoder.next_frame().unwrap_err().contains("dispose"));
}

#[test]
fn png_working_buffers_are_bounded_before_canvas_allocation() {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 8192, 8192);
        encoder.set_color(png::ColorType::Rgba);
        let writer = encoder.write_header().unwrap();
        drop(writer);
    }
    assert!(SourceDecoder::new(bytes).is_err());
}

fn webp() -> Vec<u8> {
    fn chunk(target: &mut Vec<u8>, tag: [u8; 4], data: &[u8]) {
        target.extend_from_slice(&tag);
        target.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
        target.extend_from_slice(data);
        if !data.len().is_multiple_of(2) {
            target.push(0);
        }
    }
    let mut payload = b"WEBP".to_vec();
    chunk(&mut payload, *b"VP8X", &[0x12, 0, 0, 0, 1, 0, 0, 0, 0, 0]);
    chunk(&mut payload, *b"ANIM", &[0, 0, 0, 0, 3, 0]);
    for (index, delay) in [0u8, 17, 33].into_iter().enumerate() {
        let mut encoded = Vec::new();
        let color = u8::try_from(index).unwrap() * 100;
        image::codecs::webp::WebPEncoder::new_lossless(&mut encoded)
            .encode(
                &[color, 20, 30, 255, 0, 40, 255, 128],
                2,
                1,
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        let mut frame = vec![0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, delay, 0, 0, 2];
        frame.extend_from_slice(&encoded[12..]);
        chunk(&mut payload, *b"ANMF", &frame);
    }
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(&payload);
    bytes
}

#[test]
fn animated_webp_preserves_zero_delay_order_loops_and_alpha() {
    let mut decoder = SourceDecoder::new(webp()).unwrap();
    assert!(decoder.animated());
    assert_eq!(decoder.loops(), 3);
    for (index, duration) in [0u32, 17, 33].into_iter().enumerate() {
        let frame = decoder.next_frame().unwrap().unwrap();
        assert_eq!(
            u64::from(frame.numerator) * 1000,
            u64::from(duration) * u64::from(frame.denominator)
        );
        assert_eq!(
            frame.pixels,
            [
                u8::try_from(index).unwrap() * 100,
                20,
                30,
                255,
                0,
                40,
                255,
                128
            ]
        );
    }
    assert!(decoder.next_frame().unwrap().is_none());
}

#[test]
fn framing_preflight_rejects_truncation_and_forged_frame_counts() {
    let mut png = apng(png::BitDepth::Eight, true);
    let control = png.windows(4).position(|tag| tag == b"acTL").unwrap();
    png[control + 4..control + 8].copy_from_slice(&1001u32.to_be_bytes());
    assert!(SourceDecoder::new(png).unwrap_err().contains("frame count"));
    let mut webp = webp();
    webp.pop();
    assert!(
        SourceDecoder::new(webp)
            .unwrap_err()
            .contains("RIFF length")
    );
    let mut png = apng(png::BitDepth::Eight, false);
    png.truncate(png.len() - 12); // Missing IEND must fail before returning frames.
    assert!(SourceDecoder::new(png).is_err());
}

#[test]
fn framing_preflight_bounds_aggregate_metadata_before_codec_allocation() {
    let mut webp = webp();
    let metadata = vec![0; MAX_METADATA + 1];
    webp.extend_from_slice(b"EXIF");
    webp.extend_from_slice(&u32::try_from(metadata.len()).unwrap().to_le_bytes());
    webp.extend_from_slice(&metadata);
    webp.push(0); // RIFF padding.
    let size = u32::try_from(webp.len() - 8).unwrap();
    webp[4..8].copy_from_slice(&size.to_le_bytes());
    assert!(SourceDecoder::new(webp).unwrap_err().contains("metadata"));
    let mut jpeg = vec![0xff, 0xd8];
    for _ in 0..17 {
        jpeg.extend_from_slice(&[0xff, 0xfe, 0xff, 0xff]);
        jpeg.resize(jpeg.len() + 65533, 0);
    }
    jpeg.extend_from_slice(&[0xff, 0xd9]);
    assert!(SourceDecoder::new(jpeg).unwrap_err().contains("metadata"));
}

#[test]
fn static_jpeg_survives_marker_preflight_but_truncated_jpeg_does_not() {
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut bytes)
        .encode(
            &[255, 0, 0, 0, 255, 0],
            2,
            1,
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
    let mut decoder = SourceDecoder::new(bytes.clone()).unwrap();
    assert!(!decoder.animated());
    assert_eq!(decoder.next_frame().unwrap().unwrap().pixels.len(), 8);
    assert!(decoder.next_frame().unwrap().is_none());
    bytes.truncate(bytes.len() - 2);
    assert!(
        SourceDecoder::new(bytes)
            .unwrap_err()
            .contains("end marker")
    );
}

fn png_with_info(info: png::Info<'_>, pixels: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut writer = png::Encoder::with_info(&mut bytes, info)
            .unwrap()
            .write_header()
            .unwrap();
        writer.write_image_data(pixels).unwrap();
        writer.finish().unwrap();
    }
    bytes
}

#[test]
fn color_management_applies_linear_gamma_without_changing_alpha() {
    let mut info = png::Info::with_size(1, 1);
    info.color_type = png::ColorType::Rgba;
    info.bit_depth = png::BitDepth::Eight;
    info.source_gamma = Some(png::ScaledFloat::new(1.0));
    let mut decoder = SourceDecoder::new(png_with_info(info, &[128, 64, 32, 17])).unwrap();
    let pixels = decoder.next_frame().unwrap().unwrap().pixels;
    for (sample, expected) in pixels[..3].iter().zip([188u8, 137, 99]) {
        assert!(sample.abs_diff(expected) <= 1, "{pixels:?}");
    }
    assert_eq!(pixels[3], 17);
}

#[test]
fn rgb_icc_conversion_retains_16_bit_precision_and_alpha() {
    let mut profile = moxcms::ColorProfile::new_srgb();
    profile.cicp = None;
    let linear = moxcms::ToneReprCurve::Parametric(vec![1.0]);
    profile.red_trc = Some(linear.clone());
    profile.green_trc = Some(linear.clone());
    profile.blue_trc = Some(linear);
    let mut info = png::Info::with_size(1, 1);
    info.color_type = png::ColorType::Rgba;
    info.bit_depth = png::BitDepth::Sixteen;
    info.icc_profile = Some(std::borrow::Cow::Owned(profile.encode().unwrap()));
    let pixels: Vec<_> = [32768u16, 16384, 8192, 12345]
        .into_iter()
        .flat_map(u16::to_be_bytes)
        .collect();
    let mut decoder = SourceDecoder::new(png_with_info(info, &pixels)).unwrap();
    let frame = decoder.next_frame().unwrap().unwrap();
    assert_eq!(frame.bits, 16);
    let samples: Vec<_> = frame
        .pixels
        .chunks_exact(2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .collect();
    assert!((48180..48210).contains(&samples[0]), "{samples:?}");
    assert_eq!(samples[3], 12345);
    assert_ne!(
        samples[0] % 257,
        0,
        "color conversion must not pass through 8 bits"
    );
}

#[test]
fn unsupported_or_malformed_color_profiles_are_rejected_explicitly() {
    for profile in [
        vec![0; 128],
        moxcms::ColorProfile::new_gray_with_gamma(2.2)
            .encode()
            .unwrap(),
    ] {
        let mut info = png::Info::with_size(1, 1);
        info.color_type = png::ColorType::Rgba;
        info.bit_depth = png::BitDepth::Eight;
        info.icc_profile = Some(std::borrow::Cow::Owned(profile));
        assert!(
            SourceDecoder::new(png_with_info(info, &[128, 64, 32, 17]))
                .unwrap_err()
                .contains("color profile")
        );
    }
}

#[test]
fn exif_orientation_normalizes_metadata_and_frames_at_both_precisions() {
    for bits in [png::BitDepth::Eight, png::BitDepth::Sixteen] {
        let mut info = png::Info::with_size(2, 1);
        info.color_type = png::ColorType::Rgba;
        info.bit_depth = bits;
        info.exif_metadata = Some(std::borrow::Cow::Borrowed(&[
            b'I', b'I', 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 1, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0,
        ]));
        let pixels: Vec<u8> = if bits == png::BitDepth::Sixteen {
            [12345u16, 0, 0, 65535, 0, 0, 54321, 65535]
                .into_iter()
                .flat_map(u16::to_be_bytes)
                .collect()
        } else {
            vec![123, 0, 0, 255, 0, 0, 231, 255]
        };
        let mut decoder = SourceDecoder::new(png_with_info(info, &pixels)).unwrap();
        assert_eq!((decoder.width(), decoder.height()), (1, 2));
        let frame = decoder.next_frame().unwrap().unwrap();
        assert_eq!((frame.width, frame.height), (1, 2));
        if bits == png::BitDepth::Sixteen {
            assert_eq!(
                u16::from_le_bytes([frame.pixels[0], frame.pixels[1]]),
                12345
            );
            assert_eq!(
                u16::from_le_bytes([frame.pixels[12], frame.pixels[13]]),
                54321
            );
        } else {
            assert_eq!(frame.pixels, [123, 0, 0, 255, 0, 0, 231, 255]);
        }
    }
}

#[test]
fn identical_crop_and_resize_preserves_each_animation_frame_and_timing() {
    for bits in [png::BitDepth::Eight, png::BitDepth::Sixteen] {
        let mut decoder = SourceDecoder::new(apng(bits, false)).unwrap();
        let mut count = 0;
        while let Some(mut frame) = decoder.next_frame().unwrap() {
            let timing = (frame.numerator, frame.denominator);
            let sample_bytes = usize::from(frame.bits / 8) * 4;
            // Center-based nearest-neighbor 2->1 selects the second source pixel.
            let expected = frame.pixels[sample_bytes..].to_vec();
            frame.crop_resize(0, 0, 2, 1, 1, 1).unwrap();
            assert_eq!(frame.pixels, expected);
            assert_eq!((frame.numerator, frame.denominator), timing);
            assert_eq!((frame.width, frame.height), (1, 1));
            count += 1;
        }
        assert_eq!(count, 4);
    }
}

#[test]
fn invalid_crop_is_atomic_and_rejects_overflow_and_upscaling() {
    let mut decoder = SourceDecoder::new(apng(png::BitDepth::Eight, false)).unwrap();
    let mut frame = decoder.next_frame().unwrap().unwrap();
    let original = frame.pixels.clone();
    assert!(frame.crop_resize(u32::MAX, 0, 2, 1, 1, 1).is_err());
    assert!(frame.crop_resize(0, 0, 2, 1, 3, 1).is_err());
    assert_eq!(frame.pixels, original);
    frame.crop_resize(1, 0, 1, 1, 1, 1).unwrap();
    assert_eq!(frame.pixels, [0, 0, 255, 255]);
}

#[test]
fn gif_interactive_timing_is_rejected_instead_of_silently_becoming_a_delay() {
    let mut bytes = gif(None);
    let control = bytes
        .windows(3)
        .position(|bytes| bytes == [0x21, 0xf9, 4])
        .unwrap();
    bytes[control + 3] |= 2;
    assert!(
        SourceDecoder::new(bytes)
            .unwrap_err()
            .contains("timing cannot be represented exactly")
    );
}

// Optional fixture export exercises the same maintained encoders as the native
// tests. Pixels/frames stay tiny here; production never collects an animation.
#[test]
fn export_fixtures_for_actual_wasm_round_trips() {
    let Some(directory) = std::env::var_os("CORDS_AVATAR_FIXTURES") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).unwrap();
    for (name, data) in [
        ("animated.gif", gif(Some(gif::Repeat::Finite(2)))),
        ("animated.png", apng(png::BitDepth::Eight, true)),
        ("animated16.png", apng(png::BitDepth::Sixteen, false)),
        ("animated.webp", webp()),
    ] {
        std::fs::write(directory.join(name), data).unwrap();
    }
}
