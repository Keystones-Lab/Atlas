use active_win_pos_rs::get_active_window as os_get_active_window;
use chrono::Local;
use rusqlite::{params, Connection, Result as SqlResult};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// One activity row — same shape as activity_events columns (mailbox for OS → DB → React).
#[derive(Debug, Serialize, Clone)]
pub struct ActivityEvent {
    pub started_at: String,
    pub ended_at: String,
    pub application: String,
    pub window_title: String,
    pub path: String,
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_id: Option<u64>,
}

/// Ask Windows which window has focus. Does NOT touch SQLite.
pub fn get_active_window() -> Result<ActivityEvent, String> {
    let win = os_get_active_window().map_err(|_| {
        "Could not read the foreground window (none focused, or OS denied access)".to_string()
    })?;

    Ok(ActivityEvent {
        started_at: Local::now().to_rfc3339(),
        ended_at: String::new(),
        application: win.app_name,
        window_title: win.title,
        path: win.process_path.to_string_lossy().into_owned(),
        event_type: "active_window".to_string(),
        process_id: Some(win.process_id),
    })
}

/// Write one event into activity_events. Does NOT talk to Windows.
pub fn insert_activity_event(conn: &Connection, event: &ActivityEvent) -> SqlResult<()> {
    conn.execute(
        "INSERT INTO activity_events
            (started_at, ended_at, application, window_title, path, event_type)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            event.started_at,
            event.ended_at,
            event.application,
            event.window_title,
            event.path,
            event.event_type,
        ],
    )?;
    Ok(())
}

/// Close any open activity rows (empty ended_at) when focus moves.
pub fn close_open_events(conn: &Connection, ended_at: &str) -> SqlResult<()> {
    conn.execute(
        "UPDATE activity_events SET ended_at = ?1
         WHERE ended_at IS NULL OR ended_at = ''",
        params![ended_at],
    )?;
    Ok(())
}

pub fn recent_activity(conn: &Connection, limit: i64) -> SqlResult<Vec<ActivityEvent>> {
    let mut stmt = conn.prepare(
        "SELECT started_at, ended_at, application, window_title, path, event_type
         FROM activity_events
         ORDER BY id DESC
         LIMIT ?1",
    )?;

    let rows = stmt.query_map(params![limit], |row| {
        Ok(ActivityEvent {
            started_at: row.get(0)?,
            ended_at: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            application: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            window_title: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            path: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            event_type: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
            process_id: None,
        })
    })?;

    let mut events = Vec::new();
    for row in rows {
        events.push(row?);
    }
    Ok(events)
}

/// Poll the OS in the background. Skips Atlas itself so focus is never "files | files".
/// Emits `activity-changed` only when the focused *external* window changes.
pub fn start_tracker(app: AppHandle, db: Arc<Mutex<Connection>>) {
    let our_pid = std::process::id() as u64;

    thread::spawn(move || {
        let mut last_fingerprint: Option<(String, String, String)> = None;

        loop {
            thread::sleep(Duration::from_millis(750));

            let Ok(event) = get_active_window() else {
                continue;
            };

            // Atlas is focused — keep showing the last external window; do not overwrite.
            if event.process_id == Some(our_pid) {
                continue;
            }

            let fingerprint = (
                event.application.clone(),
                event.window_title.clone(),
                event.path.clone(),
            );

            if last_fingerprint.as_ref() == Some(&fingerprint) {
                continue;
            }

            let now = Local::now().to_rfc3339();
            let mut to_emit = event.clone();
            to_emit.started_at = now.clone();

            let saved = db.lock().ok().and_then(|conn| {
                let tx = conn.unchecked_transaction().ok()?;
                close_open_events(&tx, &now).ok()?;
                insert_activity_event(&tx, &to_emit).ok()?;
                tx.commit().ok()
            });
            if saved.is_none() {
                continue;
            }

            let _ = app.emit("activity-changed", &to_emit);
            last_fingerprint = Some(fingerprint);
        }
    });
}
