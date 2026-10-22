mod account;
mod alert;
mod attachment;
mod commands;
mod db;
mod error;
mod importing;
mod investment;
mod labeling;
mod operation;
mod payee;
mod setting;
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
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let pool = tauri::async_runtime::block_on(async {
                let pool = db::open(&data_dir.join(db::DATA_FILE_NAME)).await?;
                // Applies label normalization changes to existing operations.
                labeling::renormalize_all(&pool).await?;
                // Gives a payee to operations recorded before payees existed.
                payee::assign_missing(&pool).await?;
                // Removes the files of attachments deleted with their operation.
                attachment::sweep(&pool, &data_dir.join(attachment::ATTACHMENTS_DIR)).await?;
                Ok::<_, error::AppError>(pool)
            })?;
            // Trains the category classifier ahead of the first suggestion.
            let classifier = labeling::classifier::ClassifierCache::default();
            let (cache, warm_pool) = (classifier.clone(), pool.clone());
            tauri::async_runtime::spawn(async move {
                // A failed training is tried again on the first suggestion.
                let _ = cache.refresh(&warm_pool).await;
            });
            app.manage(classifier);
            // Daily update of the security prices, once the user allowed it.
            let price_pool = pool.clone();
            tauri::async_runtime::spawn(async move {
                let today = chrono::Local::now().date_naive();
                if let Ok(true) = investment::prices::update_due(&price_pool, today).await
                    && let Ok(yahoo) = investment::yahoo::Yahoo::new()
                {
                    // Failures show on the next update from the portfolio screen.
                    let _ = investment::prices::update(&price_pool, &yahoo, today).await;
                }
            });
            app.manage(pool);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
