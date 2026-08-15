fn main() {
    const COMMANDS: &[&str] = &[
        "get_app_status",
        "save_api_key",
        "remove_api_key",
        "select_reference_image",
        "discard_reference",
        "start_generation",
        "cancel_generation",
        "save_output",
    ];

    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to build Eidos Studio");
}
