"""DB migration guide.

Commands:
sqlite3 -json rutorrent.db "SELECT * FROM state;" > output.json
--------- [Optional] Manually add the timestamps to the output.json file ---------
python db_migration.py
"""

import json
import sqlite3
import time
from typing import Any, Dict, List

SCHEMA = {
    "hash": "TEXT",
    "name": "TEXT",
    "status": "TEXT",
    "progress": "REAL DEFAULT 0.0",
    "url": "TEXT",
    "save_path": "TEXT",
    "remote_host": "TEXT",
    "remote_user": "TEXT",
    "remote_path": "TEXT",
    "rsync_timeout": "INTEGER DEFAULT 3",
    "delete_after_copy": "INTEGER DEFAULT 0",
    "in_qbit": "INTEGER DEFAULT 1",
    "files_deleted": "INTEGER DEFAULT 0",
    "added_at": "REAL",
}

with open("output.json") as f:
    rows: List[Dict[str, Any]] = json.load(f)

for row in rows:
    if not row.get("added_at"):
        timestamp = time.time()
        print(f"Adding timestamp [{timestamp}] for {row.get('name')}")
        row["added_at"] = timestamp
        time.sleep(0.75)

col_names = list(SCHEMA.keys())
col_defs = ", ".join(f"{c} {t}" for c, t in SCHEMA.items())

user_input = input(
    "Continue with DB migration? This will drop the existing 'state' table and create a new one. (y/n): "
)
if user_input.lower() != "y":
    print("DB migration aborted.")
    exit(0)

conn = sqlite3.connect("rutorrent.db")
conn.execute("DROP TABLE state")
conn.execute(f"CREATE TABLE IF NOT EXISTS state ({col_defs})")

# Only insert keys that exist in the schema (ignores extra keys in JSON)
usable = [c for c in col_names if c in rows[0]]
placeholders = ", ".join("?" for _ in usable)
sql = f"INSERT INTO state ({', '.join(usable)}) VALUES ({placeholders})"

for row in rows:
    conn.execute(sql, [row.get(c) for c in usable])

conn.commit()
conn.close()
