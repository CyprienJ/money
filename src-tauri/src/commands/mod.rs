//! Thin `#[tauri::command]` layer delegating to the domain modules.

use specta_typescript::Typescript;
use tauri_specta::{Builder, collect_commands};

use crate::error::AppError;

mod account;
mod app;
mod operation;

/// Path of the generated TypeScript bindings (`src/bindings.ts`).
const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/bindings.ts");

/// Registers every command and exported type exposed to the frontend.
pub fn builder() -> Builder<tauri::Wry> {
    Builder::new()
        .commands(collect_commands![
            app::app_version,
            account::list_accounts,
            account::list_account_summaries,
            account::get_account,
            account::create_account,
            account::update_account,
            account::set_account_archived,
            account::account_balance,
            operation::list_operations,
            operation::get_operation,
            operation::create_operation,
            operation::update_operation,
            operation::set_operation_category,
            operation::delete_operation,
            operation::list_categories,
        ])
        .typ::<AppError>()
        // Amounts are i64 cents: exact as a JS number up to 2^53 (README, stack).
        .dangerously_cast_bigints_to_number()
}

pub fn typescript() -> Typescript {
    Typescript::default().header("// @ts-nocheck")
}

/// Writes `src/bindings.ts`. Called at startup in debug builds only.
pub fn export_bindings(builder: &Builder<tauri::Wry>) {
    builder
        .export(typescript(), BINDINGS_PATH)
        .expect("failed to export TypeScript bindings");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fails when a command or type changed without regenerating `src/bindings.ts`.
    /// Regenerate with `UPDATE_BINDINGS=1 cargo test bindings` (or by running the app in dev mode).
    #[test]
    fn bindings_are_up_to_date() {
        if std::env::var_os("UPDATE_BINDINGS").is_some() {
            export_bindings(&builder());
            return;
        }

        let path = std::env::temp_dir().join(format!("money-bindings-{}.ts", std::process::id()));
        builder().export(typescript(), &path).unwrap();
        let expected = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).unwrap();

        let actual = std::fs::read_to_string(BINDINGS_PATH).unwrap_or_default();
        assert!(
            actual == expected,
            "src/bindings.ts is outdated, run `UPDATE_BINDINGS=1 cargo test bindings` to regenerate it"
        );
    }
}
