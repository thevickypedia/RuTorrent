use rusqlite::{Connection, Result};

/// Custom script to read the database on demand.
///
/// # Example
///
/// ```shell
/// ./rutorrent --read_db true
/// ```
///
/// # Sample output
///
/// ```text
/// === STATE ===
///   [08ada5a7] Sintel — Downloading (0%)
///   [dd8255ec] Big Buck Bunny — Downloading (0%)
///   [2c6b6858] Ubuntu 22.04 LTS — Downloading (0%)
///
/// === PENDING ===
///   (empty)
/// ```
pub fn print_content() -> Result<()> {
    let conn = Connection::open("rutorrent.db")?;

    println!("\n=== STATE ===");
    let mut stmt = conn.prepare("SELECT hash, name, status, progress FROM state")?;
    let mut rows = stmt.query([])?;
    let mut count = 0;
    while let Some(row) = rows.next()? {
        let hash: String = row.get(0)?;
        let name: String = row.get(1)?;
        let status: String = row.get(2)?;
        let progress: f64 = row.get(3)?;
        println!(
            "  [{}] {} — {} ({:.0}%)",
            &hash[..8],
            name,
            status,
            progress * 100.0
        );
        count += 1;
    }
    if count == 0 {
        println!("  (empty)");
    }

    println!("\n=== PENDING ===");
    let mut stmt = conn.prepare("SELECT tag, url, remote_host, remote_path FROM pending")?;
    let mut rows = stmt.query([])?;
    let mut count = 0;
    while let Some(row) = rows.next()? {
        let tag: String = row.get(0)?;
        let url: String = row.get(1)?;
        let host: String = row.get(2)?;
        let path: String = row.get(3)?;
        println!("  [{}] {}", &tag[..8], url);
        if !host.is_empty() {
            println!("        → {}:{}", host, path);
        }
        count += 1;
    }
    if count == 0 {
        println!("  (empty)");
    }

    println!();
    Ok(())
}
