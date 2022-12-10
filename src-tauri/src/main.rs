#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

fn main() {
    // https://tauri.app/v1/guides/building/macos/
    // https://github.com/tauri-apps/fix-path-env-rs
    if let Err(e) = fix_path_env::fix() {
        println!("{}", e);
    }
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
