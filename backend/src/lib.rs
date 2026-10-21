mod apps;
mod cleanup;
mod error;
mod icons;
mod paths;
mod services;
mod settings;
mod startup;
mod system;
mod tasks;
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
            app.manage(AppState {
                settings: Mutex::new(settings::load()),
                material: Mutex::new("none"),
            });
            create_main_window(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            apps::list_apps,
            cleanup::scan_cleanup,
            startup::list_startup,
            services::list_services,
            tasks::list_tasks,
            settings::get_settings,
            settings::save_settings,
            paths::storage_info,
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
