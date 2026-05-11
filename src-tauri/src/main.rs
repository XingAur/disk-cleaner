#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use disk_cleaner_tauri::cleaner::{
    self, CleanupItem, CleanupPlan, CleanupReport, ScanOptions, ScanResult,
};
use tauri::{Emitter, WebviewWindow};

type AppResult<T> = Result<T, String>;

#[tauri::command]
async fn scan_system_drive(
    window: WebviewWindow,
    options: Option<ScanOptions>,
) -> AppResult<ScanResult> {
    let progress_window = window.clone();
    tauri::async_runtime::spawn_blocking(move || {
        cleaner::scan_system_drive_with_options_and_progress(
            options.unwrap_or_default(),
            |progress| {
                let _ = progress_window.emit("scan-progress", progress);
            },
        )
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn cleanup_selected(
    window: WebviewWindow,
    items: Vec<CleanupItem>,
) -> AppResult<CleanupReport> {
    let progress_window = window.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut plan = CleanupPlan::default();
        plan.items = items
            .into_iter()
            .map(|mut item| {
                item.default_selected = true;
                item
            })
            .collect();
        cleaner::execute_cleanup_with_progress(&plan, |progress| {
            let _ = progress_window.emit("cleanup-progress", progress);
        })
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn start_drag(window: WebviewWindow) {
    let _ = window.start_dragging();
}

#[tauri::command]
fn minimize_window(window: WebviewWindow) {
    let _ = window.minimize();
}

#[tauri::command]
fn close_window(window: WebviewWindow) {
    let _ = window.close();
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            scan_system_drive,
            cleanup_selected,
            start_drag,
            minimize_window,
            close_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
