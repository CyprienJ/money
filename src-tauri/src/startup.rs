//! Opening of the data: at launch for a data file in clear, after the password for an
//! encrypted one. Until then, the pool is not managed and the frontend shows the unlock
//! screen.

use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::importing::source_document::STATEMENTS_DIR;
use crate::labeling::classifier::ClassifierCache;
use crate::{attachment, backup, importing, investment, labeling, payee};

/// Prepares the open data file, hands the pool to Tauri and starts the background tasks.
pub async fn start(app: &AppHandle, pool: SqlitePool) -> AppResult<()> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| AppError::Io(std::io::Error::other(error)))?;
    // Applies label normalization changes to existing operations.
    labeling::renormalize_all(&pool).await?;
    // Gives a payee to operations recorded before payees existed.
    payee::assign_missing(&pool).await?;
    // Removes the files of attachments deleted with their operation.
    attachment::sweep(&pool, &data_dir.join(attachment::ATTACHMENTS_DIR)).await?;
    // Removes the kept statements of cancelled or failed imports.
    importing::source_document::sweep(&pool, &data_dir.join(STATEMENTS_DIR)).await?;

    // Trains the category classifier ahead of the first suggestion.
    let (cache, warm_pool) = (app.state::<ClassifierCache>().inner().clone(), pool.clone());
    tauri::async_runtime::spawn(async move {
        // A failed training is tried again on the first suggestion.
        let _ = cache.refresh(&warm_pool).await;
    });
    // Daily full backup, unless disabled.
    let (backup_pool, dirs) = (pool.clone(), backup::Dirs::new(data_dir));
    tauri::async_runtime::spawn(async move {
        let today = chrono::Local::now().date_naive();
        if let Ok(true) = backup::full_due(&backup_pool, today).await
            && let Err(error) = backup::full(&backup_pool, &dirs, true).await
        {
            eprintln!("daily backup failed: {error}");
        }
    });
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
}
