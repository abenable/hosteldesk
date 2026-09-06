use super::support::*;
use db::DbPool;
use serde::Deserialize;
use sqlx::sqlite::SqliteConnectOptions;
use tauri::State;
use tauri::{AppHandle, Manager};
use tracing::{error, info, instrument};

/// Uses SQLite `VACUUM INTO` to produce a compact, consistent single-file copy
/// of the database. Safe to run on a live WAL database — no write lock is held
/// on the source.
///
/// Returns the absolute path to the created backup file.
#[tauri::command]
#[instrument(
    skip(app, state, license_state, auth_state),
    fields(command = "backup_database")
)]
pub async fn backup_database(
    app: AppHandle,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<String, String> {
    info!("backup_database invoked");
    require_license(&license_state)?;
    require_admin_current(&auth_state)?;

    let documents_dir = app
        .path()
        .document_dir()
        .map_err(|e| format!("Could not locate Documents folder: {e}"))?;
    let backup_dir = documents_dir.join("HostelDesk").join("backups");
    std::fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("Could not create backup directory: {e}"))?;

    let now = chrono::Local::now();
    let filename = format!("hosteldesk-{}.db", now.format("%Y%m%d-%H%M%S"));
    let backup_path = backup_dir.join(filename);
    let path_str = backup_path.to_string_lossy().to_string();

    sqlx::query("VACUUM INTO ?1")
        .bind(&path_str)
        .execute(&state.inner)
        .await
        .map_err(|e| {
            error!(error = %e, "backup_database failed");
            format!("Could not create backup: {e}")
        })?;

    info!(path = %backup_path.display(), "backup created");
    Ok(path_str)
}

/// List timestamped database backups in `Documents/HostelDesk/backups/`.
#[tauri::command]
#[instrument(skip(app, license_state), fields(command = "list_backups"))]
pub async fn list_backups(
    app: AppHandle,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<Vec<String>, String> {
    info!("list_backups invoked");
    require_license(&license_state)?;
    require_admin_current(&auth_state)?;

    let backup_dir = documents_hosteldesk_dir(&app)?.join("backups");
    if !backup_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<_> = std::fs::read_dir(&backup_dir)
        .map_err(|e| format!("could not read backups directory: {e}"))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("db"))
                .unwrap_or(false)
        })
        .collect();

    entries.sort_by_key(|entry| {
        entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });
    entries.reverse();

    let paths: Vec<String> = entries
        .into_iter()
        .map(|entry| entry.path().to_string_lossy().to_string())
        .collect();

    Ok(paths)
}

/// Request payload for [`restore_backup`].
#[derive(Debug, Deserialize)]
pub struct RestoreBackupRequest {
    pub backup_path: String,
}

/// Restore a backup file over the live database and restart the app.
///
/// The `backup_path` is validated to live inside `Documents/HostelDesk/backups/`
/// to prevent arbitrary file overwrites. The database pool is closed before the
/// copy so the replacement is clean.
#[tauri::command]
#[instrument(
    skip(app, state, license_state, auth_state),
    fields(command = "restore_backup")
)]
pub async fn restore_backup(
    request: RestoreBackupRequest,
    app: AppHandle,
    state: State<'_, DbPool>,
    license_state: State<'_, crate::LicenseState>,
    auth_state: State<'_, crate::AuthState>,
) -> Result<(), String> {
    info!(backup_path = %request.backup_path, "restore_backup invoked");
    require_license(&license_state)?;
    require_admin_current(&auth_state)?;

    let backup_dir = documents_hosteldesk_dir(&app)?.join("backups");
    let backup_path = std::path::Path::new(&request.backup_path)
        .canonicalize()
        .map_err(|e| format!("invalid backup path: {e}"))?;
    let backup_dir = backup_dir.canonicalize().unwrap_or(backup_dir);

    if !backup_path.starts_with(&backup_dir) {
        return Err("backup file must be inside the HostelDesk backups folder".to_string());
    }

    let db_path = database_path(&app)?;
    let db_path_str = db_path.to_string_lossy().to_string();
    let wal_path = std::path::PathBuf::from(format!("{}-wal", db_path_str));
    let shm_path = std::path::PathBuf::from(format!("{}-shm", db_path_str));
    let pre_restore_path = db_path.with_extension("db.pre-restore");

    validate_backup_database(&backup_path).await?;

    // Build a consistent safety copy before closing the pool. VACUUM INTO is
    // safe with WAL mode and captures all committed transactions.
    if pre_restore_path.exists() {
        std::fs::remove_file(&pre_restore_path)
            .map_err(|e| format!("could not clear pre-restore snapshot: {e}"))?;
    }
    let pre_restore_str = pre_restore_path.to_string_lossy().to_string();
    sqlx::query("VACUUM INTO ?1")
        .bind(&pre_restore_str)
        .execute(&state.inner)
        .await
        .map_err(|e| format!("could not create pre-restore snapshot: {e}"))?;

    // Close the pool so the live database file is no longer held open.
    state.inner.close().await;

    if let Err(e) = replace_database_files(&db_path, &backup_path, &wal_path, &shm_path) {
        error!(error = %e, "database restore failed; recovering pre-restore snapshot");
        let _ = std::fs::remove_file(&db_path);
        if let Err(copy_err) = std::fs::copy(&pre_restore_path, &db_path) {
            error!(
                error = %copy_err,
                "CRITICAL: could not recover pre-restore snapshot after failed restore"
            );
            return Err(format!(
                "Restore failed ({e}) and the automatic recovery copy also failed \
                 ({copy_err}). Your database from before the restore attempt is still \
                 safe at {}. Do not restart HostelDesk yet; copy that file back \
                 manually or contact support.",
                pre_restore_path.display()
            ));
        }
        let _ = std::fs::remove_file(&wal_path);
        let _ = std::fs::remove_file(&shm_path);

        info!("database recovered from pre-restore snapshot; restarting application");
        app.restart();
    }

    info!("database restored; restarting application");
    app.restart();
}

/// Validate that a backup is a readable, consistent SQLite database.
///
/// This runs before the live pool is closed so a rejected backup can never leave
/// the application in a partially restored state.
async fn validate_backup_database(path: &std::path::Path) -> Result<(), String> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .create_if_missing(false);
    let pool = sqlx::SqlitePool::connect_with(options)
        .await
        .map_err(|e| format!("not a valid SQLite backup: {e}"))?;

    let check: String = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("not a valid SQLite backup: {e}"))?;
    pool.close().await;

    if check != "ok" {
        return Err(format!(
            "not a valid SQLite backup: quick_check returned {check}"
        ));
    }
    Ok(())
}

/// Replace database files while retaining a recovery snapshot.
///
/// The caller has already validated `backup_path`. If any step fails after the
/// live pool has closed, the snapshot is copied back so the next launch opens a
/// working database.
fn replace_database_files(
    db_path: &std::path::Path,
    backup_path: &std::path::Path,
    wal_path: &std::path::Path,
    shm_path: &std::path::Path,
) -> Result<(), String> {
    let pre_restore_path = db_path.with_extension("db.pre-restore");
    let staging_path = db_path.with_extension("db.restoring");

    // The staging file is a leftover from a previous crashed restore attempt,
    // if it exists at all; a fresh restore never has one yet.
    let _ = std::fs::remove_file(&staging_path);

    std::fs::copy(backup_path, &staging_path)
        .map_err(|e| format!("could not stage backup for restore: {e}"))?;

    let _ = std::fs::remove_file(wal_path);
    let _ = std::fs::remove_file(shm_path);
    std::fs::remove_file(db_path)
        .map_err(|e| format!("could not remove live database for restore: {e}"))?;

    if let Err(e) = std::fs::rename(&staging_path, db_path) {
        let _ = std::fs::copy(&pre_restore_path, db_path);
        let _ = std::fs::remove_file(&staging_path);
        return Err(format!("could not install restored database: {e}"));
    }

    let _ = std::fs::remove_file(wal_path);
    let _ = std::fs::remove_file(shm_path);
    Ok(())
}

/// Resolve the user's HostelDesk documents directory.
fn documents_hosteldesk_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let documents_dir = app
        .path()
        .document_dir()
        .map_err(|e| format!("Could not locate Documents folder: {e}"))?;
    Ok(documents_dir.join("HostelDesk"))
}

/// Return the path to the live database file.
fn database_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("could not resolve app data dir: {e}"))?;
    Ok(data_dir.join("hosteldesk.db"))
}

/// Guard app-wide commands that affect the current hostel's database.
///
/// Backup and restore have no target-hostel request because they operate on the
/// single application database, so the session's own hostel is the target.
fn require_admin_current(auth_state: &crate::AuthState) -> Result<crate::Session, String> {
    let session = auth_state
        .current_session()
        .ok_or_else(|| "unauthorized".to_string())?;
    if session.role != "admin" {
        return Err("forbidden".to_string());
    }
    Ok(session)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::{temp_db_url, COUNTER};
    use db::DbPool;
    use std::sync::atomic::Ordering;

    #[tokio::test]
    async fn backup_database_creates_valid_copy() {
        let url = temp_db_url();
        let pool = DbPool::open(&url).await.expect("open database");

        pool.create_hostel(db::NewHostel {
            name: "Backup Test Hostel".to_string(),
            address: None,
            phone: None,
            currency: None,
            branding_logo_path: None,
        })
        .await
        .expect("create hostel");

        let backup_path = std::env::temp_dir().join(format!(
            "hosteldesk_backup_test_{}_{}.db",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let path_str = backup_path.to_string_lossy().to_string();

        sqlx::query("VACUUM INTO ?1")
            .bind(&path_str)
            .execute(&pool.inner)
            .await
            .expect("vacuum into");

        assert!(backup_path.exists(), "backup file should exist");

        let backup_url = format!("sqlite:{}", backup_path.display());
        let backup_pool = sqlx::SqlitePool::connect(&backup_url)
            .await
            .expect("open backup");
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hostels")
            .fetch_one(&backup_pool)
            .await
            .expect("count hostels");
        assert_eq!(count, 1, "backup should contain the hostel row");

        drop(backup_pool);
        let _ = std::fs::remove_file(&backup_path);
    }

    #[tokio::test]
    async fn restore_backup_does_not_replay_stale_wal() {
        let live_temp = tempfile::tempdir().expect("create live temp dir");
        let live_path = live_temp.path().join("hosteldesk.db");
        let live_url = format!("sqlite:{}", live_path.display());

        let backup_temp = tempfile::tempdir().expect("create backup temp dir");
        let backup_path = backup_temp.path().join("hosteldesk.db");
        let backup_url = format!("sqlite:{}", backup_path.display());

        // Create the backup database with a known row and flush the WAL so the
        // base file is self-contained — copying the base file alone is enough.
        {
            let pool = DbPool::open(&backup_url).await.expect("open backup");
            pool.create_hostel(db::NewHostel {
                name: "Backup Hostel".to_string(),
                address: None,
                phone: None,
                currency: None,
                branding_logo_path: None,
            })
            .await
            .expect("create backup hostel");
            let mut conn = pool.inner.acquire().await.expect("acquire connection");
            let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
                .execute(&mut *conn)
                .await;
            drop(conn);
            pool.inner.close().await;
        }

        // Create the live database with a different row. Disable auto-checkpoint
        // so the commit stays in the WAL, simulating the pre-restore state that
        // the restore fix must clear.
        {
            let pool = DbPool::open(&live_url).await.expect("open live");
            let mut conn = pool.inner.acquire().await.expect("acquire connection");
            let _ = sqlx::query("PRAGMA wal_autocheckpoint=0")
                .execute(&mut *conn)
                .await;
            let _ = sqlx::query("INSERT INTO hostels (name, currency) VALUES (?1, ?2)")
                .bind("Live Hostel")
                .bind("USD")
                .execute(&mut *conn)
                .await
                .expect("insert live hostel");
            drop(conn);
            pool.inner.close().await;
        }

        // Replace the live database exactly as replace_database_files does:
        // stage a copy, then unlink the old live file before renaming the
        // staged copy into place. Unlinking first breaks lingering file
        // descriptors — on a loaded machine, sqlx's background close can
        // finish after this point, and its final WAL checkpoint would
        // otherwise resurrect stale pages into the restored file.
        let staging_path = live_path.with_extension("db.restoring");
        std::fs::copy(&backup_path, &staging_path).expect("stage backup over live");
        std::fs::remove_file(&live_path).expect("remove live database");
        std::fs::rename(&staging_path, &live_path).expect("install restored database");
        let _ = std::fs::remove_file(format!("{}-wal", live_path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", live_path.display()));

        // No stale sidecars should remain before the next open.
        let stale_wal_exists =
            std::path::PathBuf::from(format!("{}-wal", live_path.display())).exists();
        let stale_shm_exists =
            std::path::PathBuf::from(format!("{}-shm", live_path.display())).exists();
        assert!(
            !stale_wal_exists && !stale_shm_exists,
            "restore should remove stale WAL/SHM sidecars"
        );

        // Reopening should see only the backup row; any stale WAL must not replay.
        let restored = DbPool::open(&live_url).await.expect("reopen restored");
        let names: Vec<String> = sqlx::query_scalar("SELECT name FROM hostels ORDER BY name")
            .fetch_all(&restored.inner)
            .await
            .expect("fetch hostels");
        assert_eq!(
            names,
            vec!["Backup Hostel"],
            "restored database should reflect backup, not stale WAL"
        );
    }

    #[tokio::test]
    async fn backup_validation_rejects_non_sqlite_file() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("not-a-backup.db");
        std::fs::write(&path, b"definitely not sqlite").expect("write invalid backup");

        let err = validate_backup_database(&path)
            .await
            .expect_err("invalid backup should be rejected");
        assert!(
            err.contains("not a valid SQLite backup"),
            "unexpected validation error: {err}"
        );
    }

    /// Regression test for R2: a fresh restore has no leftover `.db.restoring`
    /// staging file, and `replace_database_files` must not fail just because
    /// there is nothing to clear.
    #[tokio::test]
    async fn replace_database_files_succeeds_with_no_stale_staging_file() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let db_path = dir.path().join("hosteldesk.db");
        let backup_path = dir.path().join("backup.db");
        let wal_path = dir.path().join("hosteldesk.db-wal");
        let shm_path = dir.path().join("hosteldesk.db-shm");

        std::fs::write(&db_path, b"live database contents").expect("write live db");
        std::fs::write(&backup_path, b"backup database contents").expect("write backup");
        std::fs::write(&wal_path, b"stale wal").expect("write wal");
        std::fs::write(&shm_path, b"stale shm").expect("write shm");

        replace_database_files(&db_path, &backup_path, &wal_path, &shm_path)
            .expect("restore should succeed when no staging file is left over");

        let restored = std::fs::read(&db_path).expect("read restored db");
        assert_eq!(restored, b"backup database contents");
        assert!(!wal_path.exists(), "stale wal sidecar should be removed");
        assert!(!shm_path.exists(), "stale shm sidecar should be removed");
    }

    /// A failed restore (backup file missing) must leave the live database
    /// file untouched and report an error instead of destroying live data.
    #[tokio::test]
    async fn replace_database_files_leaves_live_db_untouched_when_backup_unreadable() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let db_path = dir.path().join("hosteldesk.db");
        let missing_backup_path = dir.path().join("does-not-exist.db");
        let wal_path = dir.path().join("hosteldesk.db-wal");
        let shm_path = dir.path().join("hosteldesk.db-shm");

        std::fs::write(&db_path, b"live database contents").expect("write live db");

        let err = replace_database_files(&db_path, &missing_backup_path, &wal_path, &shm_path)
            .expect_err("restore from a missing backup file must fail");
        assert!(
            err.contains("could not stage backup for restore"),
            "unexpected error: {err}"
        );

        let live = std::fs::read(&db_path).expect("live db must still exist");
        assert_eq!(
            live, b"live database contents",
            "live database must be untouched when staging the backup fails"
        );
    }
}
