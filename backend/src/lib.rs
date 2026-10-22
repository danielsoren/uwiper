mod apps;
mod cleanup;
mod drivers;
mod error;
mod icons;
mod leftovers;
mod log;
mod orphans;
mod paths;
mod quarantine;
mod services;
mod settings;
mod startup;
mod system;
mod targets;
mod tasks;
mod tracking;
mod uninstall;
mod util;

use std::sync::Mutex;

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

pub struct AppState {
    pub settings: Mutex<settings::Settings>,
    pub material: Mutex<&'static str>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let settings = settings::load();
            // Housekeeping: old quarantine batches and logs go away in the background.
            let for_cleanup = settings.clone();
            std::thread::spawn(move || {
                system::remove_old_autostart();
                quarantine::purge_expired(&for_cleanup);
                log::purge(&for_cleanup);
            });
            app.manage(AppState {
                settings: Mutex::new(settings),
                material: Mutex::new("none"),
            });
            create_main_window(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            apps::list_apps,
            cleanup::scan_cleanup,
            cleanup::clean_caches,
            startup::list_startup,
            startup::set_startup_enabled,
            services::list_services,
            services::set_service_start,
            tasks::list_tasks,
            tasks::set_task_enabled,
            drivers::list_drivers,
            tracking::tracking_status,
            tracking::start_tracking,
            tracking::launch_installer,
            tracking::finish_tracking,
            tracking::save_trace,
            tracking::cancel_tracking,
            tracking::list_traces,
            tracking::delete_trace,
            tracking::trace_leftovers,
            settings::get_settings,
            settings::save_settings,
            paths::storage_info,
            leftovers::scan_leftovers,
            orphans::scan_orphans,
            uninstall::create_restore_point,
            uninstall::start_uninstall,
            uninstall::uninstall_finished,
            quarantine::remove_items,
            quarantine::list_quarantine,
            quarantine::restore_batch,
            quarantine::delete_batch,
            system::environment,
            system::restart_as_admin,
            system::open_folder,
            system::reveal_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running UWiper");
}

/// The window is built here rather than in tauri.conf.json so the WebView2
/// profile lands in the UWiper folder instead of %LOCALAPPDATA%.
fn create_main_window(app: &tauri::App) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("UWiper")
        .inner_size(1280.0, 800.0)
        .min_inner_size(960.0, 620.0)
        .decorations(false)
        .transparent(true)
        .shadow(true)
        .center()
        .visible(false)
        .data_directory(paths::webview_dir())
        .build()?;
    *app.state::<AppState>().material.lock().unwrap() = system::apply_material(&window);
    window.show()?;
    Ok(())
}
