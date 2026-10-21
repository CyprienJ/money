mod account;
mod commands;
mod db;
mod error;
mod importing;
mod labeling;
mod operation;
mod payee;
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
        .invoke_handler(builder.invoke_handler())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let pool = tauri::async_runtime::block_on(async {
                let pool = db::open(&data_dir.join(db::DATA_FILE_NAME)).await?;
                // Applies label normalization changes to existing operations.
                labeling::renormalize_all(&pool).await?;
                // Gives a payee to operations recorded before payees existed.
                payee::assign_missing(&pool).await?;
                Ok::<_, error::AppError>(pool)
            })?;
            app.manage(pool);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
