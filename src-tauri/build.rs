fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_history",
            "update_dictation",
            "clear_history",
            "storage_info",
            "capture_hotkey",
            "list_rules",
            "save_rule",
            "delete_rule",
            "delete_dictation",
            "get_stats",
            "get_settings",
            "save_settings",
            "list_microphones",
            "model_status",
            "download_model",
            "copy_text",
            "permissions",
            "open_accessibility_settings",
            "open_microphone_settings",
        ]),
    ))
    .expect("tauri build");
}
