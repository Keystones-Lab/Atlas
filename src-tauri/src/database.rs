use rusqlite::{Connection, Result};
use std::path::Path;

pub fn open_connection(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.pragma_update(None, "foreign_keys", true)?;
    Ok(conn)
}

pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS documents (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            type TEXT,
            size INTEGER,
            modified_at TEXT,
            indexed_at TEXT,
            hash TEXT
        );

        CREATE TABLE IF NOT EXISTS chunks (
            id INTEGER PRIMARY KEY,
            document_id INTEGER NOT NULL,
            content TEXT NOT NULL,
            page_number INTEGER,
            slide_number INTEGER,
            line_start INTEGER,
            line_end INTEGER,
            FOREIGN KEY (document_id) REFERENCES documents(id)
        );

        CREATE TABLE IF NOT EXISTS activity_events (
            id INTEGER PRIMARY KEY,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            application TEXT,
            window_title TEXT,
            path TEXT,
            event_type TEXT
        );

        CREATE TABLE IF NOT EXISTS schema_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        ",
    )?;

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS chunks_document ON chunks(document_id);
         CREATE TABLE IF NOT EXISTS gemma2_vectors (
            chunk_id INTEGER PRIMARY KEY REFERENCES chunks(id) ON DELETE CASCADE,
            embedding BLOB NOT NULL CHECK(length(embedding) = 1024)
         );
         DROP TABLE IF EXISTS chunks_fts;",
    )?;
    Ok(())
}

/// VACUUM INTO takes a consistent snapshot, including committed WAL pages.
pub fn init(
    path: &Path,
    legacy: &[std::path::PathBuf],
) -> Result<Connection, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(path.parent().ok_or("missing database directory")?)?;
    if !path.exists() {
        if let Some(source) = legacy.iter().find(|p| p.is_file()) {
            let old =
                Connection::open_with_flags(source, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            // Publish only a finished snapshot. Concurrent launches can share the winner.
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos();
            let staging = path.with_extension(format!("{}-{now}.tmp", std::process::id()));
            std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&staging)?;
            let copy = old.execute(
                "VACUUM INTO ?1",
                [staging.to_str().ok_or("invalid database path")?],
            );
            let publish = copy.map(|_| std::fs::hard_link(&staging, path));
            std::fs::remove_file(staging)?;
            if let Err(error) = publish? {
                if error.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err(error.into());
                }
            }
        }
    }
    let conn = open_connection(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    migrate(&conn)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_launch_imports_committed_wal_rows_and_reopen_keeps_local_updates() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("atlas-migration-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("legacy.db");
        let destination = dir.join("user").join("atlas.db");
        let old = open_connection(&source).unwrap();
        old.pragma_update(None, "journal_mode", "WAL").unwrap();
        migrate(&old).unwrap();
        old.execute("INSERT INTO documents(path,name) VALUES ('a','a')", [])
            .unwrap();
        old.execute(
            "INSERT INTO activity_events(started_at) VALUES ('time')",
            [],
        )
        .unwrap();
        {
            let db = init(&destination, std::slice::from_ref(&source)).unwrap();
            assert_eq!(crate::activity::recent_activity(&db, 12).unwrap().len(), 1);
            db.execute("INSERT INTO documents(path,name) VALUES ('b','b')", [])
                .unwrap();
        }
        let db = init(&destination, std::slice::from_ref(&source)).unwrap();
        assert_eq!(crate::indexer::list_documents(&db, 400).unwrap().len(), 2);
        assert_eq!(crate::indexer::list_documents(&old, 400).unwrap().len(), 1);
        drop(db);
        drop(old);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
