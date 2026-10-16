use super::*;

#[test]
fn fuel_and_imported_memory_limits_fail_closed() -> Result<()> {
    let bytes = include_bytes!("../tests/fixtures/animated.png.jxl");
    assert!(validate_bounded(bytes, 1).is_err());
    let (mut store, _instance, memory) = instantiate(2_000_000_000)?;
    assert!(memory.grow(&mut store, 2048).is_err());
    assert!(memory.data_size(&store) <= HEAP);
    Ok(())
}

#[test]
fn native_incremental_frames_preserve_metadata_precision_and_pixels() -> Result<()> {
    for bytes in [
        include_bytes!("../tests/fixtures/animated.png.jxl").as_slice(),
        include_bytes!("../tests/fixtures/animated16.png.jxl").as_slice(),
        include_bytes!("../tests/fixtures/animated.gif.jxl").as_slice(),
        include_bytes!("../tests/fixtures/animated.webp.jxl").as_slice(),
    ] {
        let expected = validate(bytes)?;
        let mut decoder = playback::Decoder::new(bytes)?;
        let mut durations = Vec::new();
        let mut last = false;
        while let Some(frame) = decoder.next_frame()? {
            assert!(!last);
            assert_eq!(
                (frame.width, frame.height, frame.bits),
                (expected.width, expected.height, expected.bits)
            );
            assert_eq!(
                frame.pixels.len(),
                (frame.width * frame.height * 4 * (frame.bits / 8)) as usize
            );
            assert_eq!(frame.loops, expected.loops);
            assert_eq!(frame.ticks_numerator, expected.ticks_per_second_numerator);
            assert_eq!(
                frame.ticks_denominator,
                expected.ticks_per_second_denominator
            );
            durations.push(frame.duration_ticks);
            last = frame.last;
        }
        assert!(last);
        assert_eq!(durations, expected.durations);
        assert!(decoder.next_frame()?.is_none());
    }
    let mut decoder = playback::Decoder::new(include_bytes!("../tests/fixtures/looping-a.jxl"))?;
    let frame = decoder
        .next_frame()?
        .ok_or_else(|| anyhow::anyhow!("missing first frame"))?;
    assert_eq!((frame.width, frame.height, frame.bits), (64, 64, 8));
    for (index, pixel) in frame.pixels.chunks_exact(4).enumerate() {
        let (x, y) = (index % 64, index / 64);
        let expected = if (4..12).contains(&x) && (12..52).contains(&y) {
            [245, 245, 245, 255]
        } else {
            [40, 80, 210, 255]
        };
        assert_eq!(pixel, expected);
    }
    assert!(playback::Decoder::new(b"malformed").is_err());
    Ok(())
}
