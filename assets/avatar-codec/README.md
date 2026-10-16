# Bundled avatar codec

These are the reviewed, pinned libjxl and Rust source-decoder artifacts used by
the desktop workers and restricted native server validator. Builds verify their
SHA-256 pins before copying or embedding them; processing never downloads code.

`cords-jxl.manifest.json` records the original codec build inputs and artifact
hashes. It is provenance for that build, not a claim that unrelated later Cargo
workspace changes were included in the WASM. Codec sources and repeatable build
instructions are maintained in `tools/avatar-codec/` and
`crates/cords-avatar-source/`. The bindings are generated, not hand edited.

Third-party license and patent notices are in `licenses/`; the Cords source
decoder remains covered by the repository's AGPL-3.0-only license. Rust notices
include the source-decoder dependency closure (including tiny test encoders).
