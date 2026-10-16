use sha2::{Digest as _, Sha256};
use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=CORDS_AVATAR_CODEC_DIR");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = env::var_os("CORDS_AVATAR_CODEC_DIR")
        .map_or_else(|| root.join("assets/avatar-codec"), PathBuf::from);
    let path = directory.join("cords-jxl.wasm");
    println!("cargo:rerun-if-changed={}", path.display());
    let bytes = fs::read(&path).unwrap_or_else(|error| {
        panic!("Pinned avatar codec missing at {}: {error}. Build tools/avatar-codec first or set CORDS_AVATAR_CODEC_DIR", path.display())
    });
    assert_eq!(
        hex::encode(Sha256::digest(&bytes)),
        "5c98d827b926297039c5da9a7881a6bfaffe922ab7d2f2532fe4641a5b64f5c4",
        "Avatar codec hash mismatch; rebuild and review pinned artifact before changing it"
    );
    let output = PathBuf::from(env::var_os("OUT_DIR").ok_or("Cargo must set OUT_DIR")?);
    fs::write(output.join("cords-jxl.wasm"), bytes)?;
    Ok(())
}
