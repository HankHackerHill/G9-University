fn main() {
    // A release-profile build without tauri's `custom-protocol` feature is a
    // development binary: it loads its interface from `build.devUrl`
    // (http://localhost:5173) and treats that origin as local, so whatever listens
    // on the port gets IPC access. `npm run tauri:build` enables the feature; a bare
    // `cargo build --release` does not (security audit 2026-11-12). `is_dev()` is
    // the flag tauri's own build script exports (`DEP_TAURI_DEV`); `build()` below
    // reads it too.
    if std::env::var("PROFILE").as_deref() == Ok("release") && tauri_build::is_dev() {
        eprintln!(
            "error: refusing a release build of the desktop shell in Tauri development mode \
             (it would load its UI from http://localhost:5173). Build it with `npm run tauri:build` \
             in apps/desktop, or `cargo build --release -p g9-mentor-desktop --features tauri/custom-protocol`."
        );
        std::process::exit(1);
    }
    tauri_build::build()
}
