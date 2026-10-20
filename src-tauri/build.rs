fn main() {
    // Rebuild when a migration is added, since `sqlx::migrate!` embeds them at compile time.
    println!("cargo:rerun-if-changed=migrations");
    tauri_build::build()
}
