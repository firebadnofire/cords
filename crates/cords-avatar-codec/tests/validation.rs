use cords_avatar_codec::validate;

#[test]
fn emitted_wasm_validates_incremental_animated_outputs() -> anyhow::Result<()> {
    for input in [
        include_bytes!("fixtures/animated.gif.jxl").as_slice(),
        include_bytes!("fixtures/animated.png.jxl").as_slice(),
        include_bytes!("fixtures/animated16.png.jxl").as_slice(),
        include_bytes!("fixtures/animated.webp.jxl").as_slice(),
    ] {
        let info = validate(input)?;
        assert_eq!((info.width, info.height), (1, 1));
        assert_eq!(info.loops, 3);
        assert!(info.frames > 1);
        assert_eq!(info.frames as usize, info.durations.len());
        assert!(matches!(info.bits, 8 | 16));
    }
    Ok(())
}

#[test]
fn malformed_and_truncated_inputs_cannot_validate() {
    assert!(validate(&[]).is_err());
    assert!(validate(&vec![0; 5_000_001]).is_err());
    assert!(validate(&[1, 2, 3, 4]).is_err());
    let input = include_bytes!("fixtures/animated.png.jxl");
    for length in [1, input.len() / 2, input.len() - 1] {
        assert!(validate(&input[..length]).is_err(), "truncated at {length}");
    }
}
