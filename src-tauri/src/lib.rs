mod account;
mod alert;
mod amount;
mod attachment;
mod backup;
mod commands;
mod currency;
mod db;
mod error;
mod importing;
mod investment;
mod labeling;
mod operation;
mod payee;
mod setting;
mod startup;
mod stats;
mod tag;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = commands::builder();

    #[cfg(debug_assertions)]
    commands::export_bindings(&builder);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(builder.invoke_handler())
        .setup(|app| {
            let data_dir = startup::data_dir(app.handle())?;
            std::fs::create_dir_all(&data_dir)?;
            // A restore chosen in the settings replaces the files before they are opened.
            if let Err(error) = backup::apply_pending_restore(&data_dir) {
                eprintln!("restore failed, the current data is kept: {error}");
            }
            app.manage(labeling::classifier::ClassifierCache::default());
            let data_file = data_dir.join(db::DATA_FILE_NAME);
            // An encrypted data file waits for its password (unlock command).
            if !db::encryption::is_encrypted(&data_file)? {
                let handle = app.handle().clone();
                tauri::async_runtime::block_on(async {
                    let pool = db::open(&data_file).await?;
                    startup::start(&handle, pool).await
                })?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
