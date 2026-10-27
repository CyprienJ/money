use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::error::AppError;

/// Data directory of a test, deleted afterwards.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let name = format!(
            "money-backup-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }

    fn dirs(&self) -> Dirs {
        Dirs::new(self.0.clone())
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn read(&self, relative: &str) -> String {
        std::fs::read_to_string(self.0.join(relative)).unwrap()
    }
}

/// A pool on a file of the test directory: `VACUUM INTO` needs a file database.
async fn file_pool(temp: &TempDir) -> SqlitePool {
    crate::db::open(&temp.0.join("pool.sqlite")).await.unwrap()
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn copies_the_data_and_the_attachments() {
    tauri::async_runtime::block_on(async {
        let temp = TempDir::new();
        let pool = file_pool(&temp).await;
        temp.write("attachments/1-facture.pdf", "pdf");
        let dirs = temp.dirs();

        assert!(full_due(&pool, Local::now().date_naive()).await.unwrap());
        let backup = full(&pool, &dirs, true).await.unwrap();
        assert_eq!(backup.kind, BackupKind::Full);
        assert_eq!(backup.attachment_count, 1);
        let dir = PathBuf::from(&backup.path);
        assert!(dir.starts_with(&dirs.default_backups));
        assert!(dir.join(DATA_FILE_NAME).is_file());
        assert_eq!(
            std::fs::read_to_string(dir.join("attachments/1-facture.pdf")).unwrap(),
            "pdf"
        );
        assert!(!full_due(&pool, Local::now().date_naive()).await.unwrap());

        // Two backups in the same second get a suffix.
        let second = full(&pool, &dirs, true).await.unwrap();
        assert_ne!(second.path, backup.path);
        assert_eq!(list(&pool, &dirs).await.unwrap().len(), 2);
    });
}

#[test]
fn uses_the_chosen_folder_and_can_be_disabled() {
    tauri::async_runtime::block_on(async {
        let temp = TempDir::new();
        let pool = file_pool(&temp).await;
        let dirs = temp.dirs();
        let defaults = settings(&pool, &dirs).await.unwrap();
        assert!(defaults.daily && !defaults.custom_folder);

        let chosen = temp.0.join("disque externe");
        set_folder(&pool, Some(chosen.to_str().unwrap()))
            .await
            .unwrap();
        let backup = full(&pool, &dirs, true).await.unwrap();
        assert!(PathBuf::from(&backup.path).starts_with(&chosen));
        assert!(settings(&pool, &dirs).await.unwrap().custom_folder);

        // A file is not a folder.
        temp.write("fichier", "");
        let file = temp.0.join("fichier");
        assert!(matches!(
            set_folder(&pool, Some(file.to_str().unwrap())).await,
            Err(AppError::Backup(BackupError::FolderUnavailable(_)))
        ));
        set_folder(&pool, None).await.unwrap();
        assert!(!settings(&pool, &dirs).await.unwrap().custom_folder);

        set_daily(&pool, false).await.unwrap();
        assert!(!full_due(&pool, NaiveDate::MAX).await.unwrap());
    });
}

#[test]
fn keeps_the_most_recent_backups() {
    let temp = TempDir::new();
    let folder = temp.0.join("backups");
    for day in 10..22 {
        temp.write(
            &format!("backups/complete-202610{day}-090000/money.sqlite"),
            "",
        );
    }
    for day in 1..=25 {
        temp.write(&format!("backups/money-202609{day:02}-120000.sqlite"), "");
    }
    // Not a backup: never deleted.
    temp.write("backups/notes.txt", "");

    rotate_full(&folder, 10).unwrap();
    rotate_import(&folder, 20).unwrap();
    let names = names(&folder);
    assert_eq!(
        names.iter().filter(|n| n.starts_with("complete-")).count(),
        10
    );
    assert!(!names.contains(&"complete-20261011-090000".to_owned()));
    assert!(names.contains(&"complete-20261012-090000".to_owned()));
    assert_eq!(names.iter().filter(|n| n.starts_with("money-")).count(), 20);
    assert!(!names.contains(&"money-20260905-120000.sqlite".to_owned()));
    assert!(names.contains(&"notes.txt".to_owned()));
}

#[test]
fn restores_a_full_backup_at_the_next_start() {
    tauri::async_runtime::block_on(async {
        let temp = TempDir::new();
        let pool = file_pool(&temp).await;
        let dirs = temp.dirs();
        temp.write("backups/complete-20261001-090000/money.sqlite", "old data");
        temp.write(
            "backups/complete-20261001-090000/attachments/1-old.pdf",
            "old",
        );
        temp.write("money.sqlite", "current data");
        temp.write("money.sqlite-journal", "journal");
        temp.write("attachments/2-new.pdf", "new");
        // Joined part by part: the path must be written as `list` writes it, on Windows too.
        let source = temp.0.join("backups").join("complete-20261001-090000");

        assert!(matches!(
            schedule_restore(&pool, &dirs, "/etc/passwd").await,
            Err(AppError::Backup(BackupError::NotABackup(_)))
        ));
        schedule_restore(&pool, &dirs, source.to_str().unwrap())
            .await
            .unwrap();
        // The current state is backed up first.
        assert_eq!(list(&pool, &dirs).await.unwrap().len(), 2);

        assert_eq!(apply_pending_restore(&temp.0).unwrap(), Some(source));
        assert_eq!(temp.read("money.sqlite"), "old data");
        assert!(!temp.0.join("money.sqlite-journal").exists());
        assert_eq!(names(&temp.0.join("attachments")), ["1-old.pdf"]);
        // Done once.
        assert_eq!(apply_pending_restore(&temp.0).unwrap(), None);
    });
}

#[test]
fn restores_the_data_only_of_a_backup_made_before_an_import() {
    let temp = TempDir::new();
    temp.write("backups/money-20261001-090000.sqlite", "before import");
    temp.write("money.sqlite", "current data");
    temp.write("attachments/2-new.pdf", "new");
    let source = temp.0.join("backups/money-20261001-090000.sqlite");
    std::fs::write(temp.0.join(RESTORE_MARKER), source.to_str().unwrap()).unwrap();

    apply_pending_restore(&temp.0).unwrap();
    assert_eq!(temp.read("money.sqlite"), "before import");
    assert_eq!(names(&temp.0.join("attachments")), ["2-new.pdf"]);
}

#[test]
fn reads_the_date_of_a_backup_from_its_name() {
    let at = |name: &str| stamp_of(name).map(|d| d.to_string());
    assert_eq!(
        at("complete-20261022-093000-2"),
        Some("2026-10-22 09:30:00".to_owned())
    );
    assert_eq!(
        at("money-20261022-093000.sqlite"),
        Some("2026-10-22 09:30:00".to_owned())
    );
    assert_eq!(at("complete-hier"), None);
}
