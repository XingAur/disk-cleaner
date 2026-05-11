#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(test))]
mod app {
    use disk_cleaner_tauri::cleaner::{self, CleanupPlan, CleanupReport, ScanOptions, ScanResult};
    use std::sync::Mutex;
    use tauri::{Emitter, State, WebviewWindow};

    type AppResult<T> = Result<T, String>;

    #[derive(Default)]
    struct LastScan {
        plan: Mutex<Option<CleanupPlan>>,
    }

    #[tauri::command]
    async fn scan_system_drive(
        window: WebviewWindow,
        state: State<'_, LastScan>,
        options: Option<ScanOptions>,
    ) -> AppResult<ScanResult> {
        let progress_window = window.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            cleaner::scan_system_drive_with_options_and_progress(
                options.unwrap_or_default(),
                |progress| {
                    let _ = progress_window.emit("scan-progress", progress);
                },
            )
        })
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;

        *state
            .plan
            .lock()
            .map_err(|_| "scan state is unavailable".to_string())? = Some(result.plan.clone());

        Ok(result)
    }

    #[tauri::command]
    async fn cleanup_selected(
        window: WebviewWindow,
        state: State<'_, LastScan>,
        item_ids: Vec<String>,
    ) -> AppResult<CleanupReport> {
        let latest_plan = state
            .plan
            .lock()
            .map_err(|_| "scan state is unavailable".to_string())?
            .clone()
            .ok_or_else(|| "Please scan before cleanup.".to_string())?;
        let items = cleaner::selected_items_from_plan(&latest_plan, &item_ids)?;
        let progress_window = window.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let mut plan = CleanupPlan::default();
            plan.items = items;
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

    pub fn run() {
        tauri::Builder::default()
            .manage(LastScan::default())
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
}

#[cfg(not(test))]
fn main() {
    app::run();
}

#[cfg(test)]
fn main() {}
