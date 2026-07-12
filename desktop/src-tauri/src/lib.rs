mod settings;

pub use settings::SettingsStore;

use tauri::{Manager, State};

#[tauri::command]
fn get_setting(store: State<'_, SettingsStore>, key: String) -> Result<Option<String>, String> {
    store.get(&key).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_setting(store: State<'_, SettingsStore>, key: String, value: String) -> Result<(), String> {
    store.set(&key, &value).map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(all(debug_assertions, feature = "e2e"))]
    let builder = builder
        .plugin(tauri_plugin_wdio::init())
        .plugin(tauri_plugin_wdio_webdriver::init());

    builder
        .setup(|app| {
            let application_data = app.path().app_data_dir()?;
            std::fs::create_dir_all(&application_data)?;
            let settings = SettingsStore::open(application_data.join("luna.db"))?;
            app.manage(settings);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_setting, set_setting])
        .run(tauri::generate_context!())
        .expect("Luna failed to start");
}
