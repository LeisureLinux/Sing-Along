fn main() {
    tauri_build::build();

    // `tauri_build::build()` emits `rerun-if-changed` for the Tauri config and
    // capabilities, but not for `build.frontendDist`. Cargo therefore considers
    // the crate unchanged when only the frontend was rebuilt, and skips this
    // build script -- so the binary keeps embedding whatever `dist` looked like
    // the first time it was compiled.
    //
    // That silently ships a stale UI: `npm run tauri build` regenerates `dist`,
    // the Rust side is untouched, and the release binary still serves the
    // previous frontend. Pointing `rerun-if-changed` at the embedded directory
    // makes cargo re-embed it whenever the frontend build output changes.
    //
    // Resolved the same way tauri-build resolves `frontendDist`: relative to
    // `tauri.conf.json`'s directory (`src-tauri`).
    let frontend_dist = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../dist");
    println!("cargo:rerun-if-changed={}", frontend_dist.display());
}
