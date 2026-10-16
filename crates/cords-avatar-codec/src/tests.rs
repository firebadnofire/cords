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
