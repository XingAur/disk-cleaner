#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use disk_cleaner_tauri::inventory::{self, model::*, platform, rules, Plan};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::{Emitter, Manager, State, WebviewWindow};

#[derive(Default)]
struct Session {
    plan: Mutex<Plan>,
    busy: AtomicBool,
    cancel: AtomicBool,
}
type Shared = Arc<Session>;
struct BusyGuard(Shared);
impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.0.busy.store(false, Ordering::SeqCst);
    }
}
fn acquire(state: &Shared) -> Result<BusyGuard, String> {
    state
        .busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| "请等待当前操作结束".to_string())?;
    Ok(BusyGuard(state.clone()))
}
#[tauri::command]
async fn scan_inventory(
    window: WebviewWindow,
    state: State<'_, Shared>,
    options: ScanOptions,
) -> Result<Inventory, String> {
    let guard = acquire(&state)?;
    state.cancel.store(false, Ordering::SeqCst);
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let roots = rules::Roots::current()?;
        let mut rules = rules::default_rules(&roots);
        rules.extend(rules::registered_rules(&options.registrations)?);
        rules.extend(rules::large_file_rules(&roots));
        let (result, plan) = inventory::scan(rules, &state.cancel, |p| {
            let _ = window.emit("scan-progress", p);
        });
        *state.plan.lock().map_err(|_| "扫描状态不可用")? = plan;
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn cancel_scan(state: State<'_, Shared>) {
    state.cancel.store(true, Ordering::SeqCst);
}
#[tauri::command]
async fn cleanup_inventory(
    window: WebviewWindow,
    state: State<'_, Shared>,
    request: CleanupRequest,
) -> Result<CleanupReport, String> {
    let guard = acquire(&state)?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let mut plan = state.plan.lock().map_err(|_| "扫描状态不可用")?;
        let result = inventory::cleanup(&plan, &request, |p| {
            let _ = window.emit("cleanup-progress", p);
        })?;
        *plan = Plan::default();
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn browse_item(state: State<'_, Shared>, id: String) -> Result<Vec<Item>, String> {
    let guard = acquire(&state)?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let mut plan = state.plan.lock().map_err(|_| "扫描状态不可用")?;
        inventory::browse(&mut plan, &id)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn preview_item(state: State<'_, Shared>, id: String) -> Result<String, String> {
    let guard = acquire(&state)?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let plan = state.plan.lock().map_err(|_| "扫描状态不可用")?;
        inventory::preview(&plan, &id)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn reveal_item(state: State<'_, Shared>, id: String) -> Result<(), String> {
    let plan = state.plan.lock().map_err(|_| "扫描状态不可用")?;
    platform::reveal(&plan.entries.get(&id).ok_or("未知条目")?.item.path)
}
#[tauri::command]
fn open_system(target: String) -> Result<(), String> {
    platform::open_system(&target)
}
#[tauri::command]
async fn empty_recycle_bin(state: State<'_, Shared>, confirmation: String) -> Result<(), String> {
    if confirmation != "清空回收站" {
        return Err("请明确确认清空全部磁盘回收站".into());
    }
    let guard = acquire(&state)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        platform::empty_recycle_bin()
    })
    .await
    .map_err(|e| e.to_string())?
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
fn hide_to_tray(window: WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}
#[tauri::command]
fn maximize_window(window: WebviewWindow) {
    if window.is_maximized().unwrap_or(false) {
        let _ = window.unmaximize();
    } else {
        let _ = window.maximize();
    }
}
#[tauri::command]
fn close_window(window: WebviewWindow) {
    let _ = window.close();
}
fn main() {
    tauri::Builder::default()
        .manage(Shared::default())
        .setup(|app| {
            use tauri::{
                menu::{Menu, MenuItem},
                tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
            };
            let show = MenuItem::with_id(app, "show", "打开磁盘管家", true, None::<&str>)?;
            let hide = MenuItem::with_id(app, "hide", "隐藏到托盘", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &hide, &quit])?;
            TrayIconBuilder::with_id("main-tray")
                .icon(
                    app.default_window_icon()
                        .expect("bundled application icon")
                        .clone(),
                )
                .icon_as_template(false)
                .tooltip("磁盘管家 · Disk Cleaner")
                .menu(&menu)
                .menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main_window(app),
                    "hide" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.hide();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                    ) {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_inventory,
            cancel_scan,
            cleanup_inventory,
            browse_item,
            preview_item,
            reveal_item,
            open_system,
            empty_recycle_bin,
            start_drag,
            minimize_window,
            hide_to_tray,
            maximize_window,
            close_window
        ])
        .run(tauri::generate_context!())
        .expect("无法启动磁盘管家");
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
