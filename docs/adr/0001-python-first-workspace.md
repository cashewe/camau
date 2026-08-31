# Use a Python-first Cargo workspace

Camau presents a Python-only supported API, so its installable package lives in `src/camau` while Rust implementation is divided between an unpublished pure core crate and a thin PyO3 binding crate under `crates/`. This adds workspace structure, but makes the Python entry point obvious and enforces the runtime boundary through Cargo dependencies instead of naming conventions.
