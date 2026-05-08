use crate::settings::{PutItem, RsyncTrack, Status};
use rusqlite::{params, Connection};
use std::collections::HashMap;

/// Opens (or creates) the SQLite database and ensures the schema exists.
pub fn open() -> Connection {
    let conn = Connection::open("rutorrent.db").expect("Failed to open database");
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS state (
            hash        TEXT PRIMARY KEY,
            name        TEXT NOT NULL,
            status      TEXT NOT NULL,
            progress    REAL NOT NULL DEFAULT 0.0,
            url         TEXT NOT NULL,
            save_path   TEXT NOT NULL,
            remote_host TEXT NOT NULL,
            remote_user TEXT NOT NULL,
            remote_path TEXT NOT NULL,
            delete_after_copy INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS pending (
            tag         TEXT PRIMARY KEY,
            url         TEXT NOT NULL,
            save_path   TEXT NOT NULL,
            remote_host TEXT NOT NULL,
            remote_user TEXT NOT NULL,
            remote_path TEXT NOT NULL,
            delete_after_copy INTEGER NOT NULL DEFAULT 0
        );",
    )
    .expect("Failed to create schema");
    conn
}

/// Inserts or replaces a tracked torrent entry.
pub fn upsert(conn: &Connection, hash: &str, entry: &RsyncTrack) {
    let (status, progress) = encode_status(&entry.status);
    conn.execute(
        "INSERT OR REPLACE INTO state
            (hash, name, status, progress, url, save_path, remote_host, remote_user, remote_path, delete_after_copy)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            hash,
            entry.name,
            status,
            progress,
            entry.put_item.url,
            entry.put_item.save_path,
            entry.put_item.remote_host,
            entry.put_item.remote_username,
            entry.put_item.remote_path,
            entry.put_item.delete_after_copy as i32,
        ],
    )
        .expect("Failed to upsert state");
}

pub fn upsert_pending(conn: &Connection, tag: &str, item: &PutItem) {
    conn.execute(
        "INSERT OR REPLACE INTO pending
            (tag, url, save_path, remote_host, remote_user, remote_path, delete_after_copy)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            tag,
            item.url,
            item.save_path,
            item.remote_host,
            item.remote_username,
            item.remote_path,
            item.delete_after_copy as i32,
        ],
    )
    .expect("Failed to upsert pending");
}

pub fn remove_pending(conn: &Connection, tag: &str) {
    conn.execute("DELETE FROM pending WHERE tag = ?1", params![tag])
        .expect("Failed to remove pending");
}

pub fn load_pending(conn: &Connection) -> HashMap<String, PutItem> {
    let mut stmt = conn
        .prepare("SELECT tag, url, save_path, remote_host, remote_user, remote_path, delete_after_copy FROM pending")
        .expect("Failed to prepare load_pending query");

    stmt.query_map([], |row| {
        let tag: String = row.get(0)?;
        let item = PutItem {
            url: row.get(1)?,
            name: None,
            hash: None,
            trackers: None,
            save_path: row.get(2)?,
            remote_host: row.get(3)?,
            remote_username: row.get(4)?,
            remote_path: row.get(5)?,
            delete_after_copy: row.get::<_, i32>(6)? != 0,
        };
        Ok((tag, item))
    })
    .expect("Failed to query pending")
    .filter_map(|r| r.ok())
    .collect()
}

/// Removes a torrent entry by hash.
pub fn remove(conn: &Connection, hash: &str) {
    conn.execute("DELETE FROM state WHERE hash = ?1", params![hash])
        .expect("Failed to remove state");
}

/// Loads all persisted entries back into a HashMap on startup.
pub fn load_all(conn: &Connection) -> HashMap<String, RsyncTrack> {
    let mut stmt = conn
        .prepare("SELECT hash, name, status, progress, url, save_path, remote_host, remote_user, remote_path, delete_after_copy FROM state")
        .expect("Failed to prepare load query");

    stmt.query_map([], |row| {
        let hash: String = row.get(0)?;
        let name: String = row.get(1)?;
        let status_str: String = row.get(2)?;
        let progress: f64 = row.get(3)?;
        let url: String = row.get(4)?;
        let save_path: String = row.get(5)?;
        let remote_host: String = row.get(6)?;
        let remote_username: String = row.get(7)?;
        let remote_path: String = row.get(8)?;
        let delete_after_copy: i32 = row.get(9)?;

        let status = decode_status(&status_str, progress);
        let put_item = PutItem {
            url,
            name: None,
            hash: None,
            trackers: None,
            save_path,
            remote_host,
            remote_username,
            remote_path,
            delete_after_copy: delete_after_copy != 0,
        };

        Ok((
            hash,
            RsyncTrack {
                name,
                status,
                put_item,
            },
        ))
    })
    .expect("Failed to query state")
    .filter_map(|r| r.ok())
    .collect()
}

fn encode_status(status: &Status) -> (&'static str, f64) {
    match status {
        Status::Downloading(p) => ("Downloading", *p),
        Status::Copying(p) => ("Copying", *p),
        Status::Completed => ("Completed", 1.0),
        Status::Failed => ("Failed", 0.0),
    }
}

fn decode_status(s: &str, progress: f64) -> Status {
    match s {
        "Copying" => Status::Copying(progress),
        "Completed" => Status::Completed,
        "Failed" => Status::Failed,
        _ => Status::Downloading(progress),
    }
}
