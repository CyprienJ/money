//! Thin `#[tauri::command]` layer delegating to the domain modules.

use specta_typescript::Typescript;
use tauri_specta::{Builder, collect_commands};

use crate::error::AppError;

mod account;
mod app;
mod attachment;
mod csv_mapping;
mod import;
mod operation;
mod payee;
mod stats;
mod tag;

/// Path of the generated TypeScript bindings (`src/bindings.ts`).
const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/bindings.ts");

/// Registers every command and exported type exposed to the frontend.
pub fn builder() -> Builder<tauri::Wry> {
    Builder::new()
        .commands(collect_commands![
            app::app_version,
            app::data_location,
            account::list_accounts,
            account::list_account_summaries,
            account::get_account,
            account::create_account,
            account::update_account,
            account::set_account_archived,
            account::account_balance,
            account::list_valuations,
            account::create_valuation,
            account::update_valuation,
            account::delete_valuation,
            operation::search_operations,
            operation::list_transfer_proposals,
            operation::detect_transfers,
            operation::link_transfer,
            operation::unlink_transfer,
            operation::dismiss_transfer,
            operation::transfer_partner,
            operation::transfer_link_candidates,
            operation::get_operation,
            operation::create_operation,
            operation::update_operation,
            operation::set_operation_category,
            operation::same_label_operations,
            operation::categorize_same_label,
            operation::list_rules,
            operation::count_rule_matches,
            operation::create_rule,
            operation::update_rule,
            operation::delete_rule,
            operation::reorder_rules,
            operation::apply_rule,
            operation::delete_operation,
            operation::list_categories,
            operation::create_category,
            operation::update_category,
            operation::delete_category,
            operation::list_scheduled_operations,
            operation::create_scheduled_operation,
            operation::update_scheduled_operation,
            operation::delete_scheduled_operation,
            operation::upcoming_operations,
            operation::schedule_occurrences,
            operation::list_schedule_match_proposals,
            operation::detect_schedule_matches,
            operation::match_schedule_occurrence,
            operation::unmatch_schedule_occurrence,
            operation::dismiss_schedule_match,
            operation::schedule_match_candidates,
            payee::list_payees,
            payee::rename_payee,
            payee::merge_payees,
            payee::detach_payee_label,
            tag::list_tags,
            tag::rename_tag,
            tag::delete_tag,
            attachment::list_attachments,
            attachment::add_attachment,
            attachment::delete_attachment,
            attachment::open_attachment,
            csv_mapping::list_csv_mappings,
            csv_mapping::get_csv_mapping,
            csv_mapping::create_csv_mapping,
            csv_mapping::update_csv_mapping,
            csv_mapping::delete_csv_mapping,
            csv_mapping::suggest_csv_mappings,
            csv_mapping::detect_csv_mapping,
            import::list_import_batches,
            import::cancel_import,
            import::read_file_head,
            import::preview_import,
            import::import_statement,
            import::supported_banks,
            import::detect_pdf_statement,
            stats::expenses_by_category,
            stats::income_vs_expenses,
            stats::balance_history,
            stats::net_worth_at,
            stats::net_worth_history,
            stats::compare_expenses_by_category,
            stats::top_payees,
            stats::largest_expenses,
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
