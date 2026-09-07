## Troubleshooting Guide
In case of external modifications `RuTorrent` might panic or get stuck in an unrecoverable state.
Use this guide to perform a manual override on the database and update records to their latest state.

### Pre-requisite

**Linux**
```shell
sudo apt install sqlite3
```

**macOS**
```shell
brew install sqlite3
```

### Read Database

```shell
sqlite3 -json rutorrent.db "SELECT * FROM state;" | jq .
```

### Clear `pending` records

```shell
sqlite3 rutorrent.db "DELETE FROM pending WHERE hash=;" | jq .
```

### Modify Database

##### Export

```shell
sqlite3 -json rutorrent.db "SELECT * FROM state;" | jq . > state.json
```

##### Import

**Verify Schema**
```shell
sqlite3 rutorrent.db ".schema state"
```

**Import data from JSON file**
```shell
sqlite3 rutorrent.db <<'SQL'
BEGIN;

DELETE FROM state;

INSERT INTO state (
    hash,
    name,
    status,
    progress,
    url,
    save_path,
    remote_host,
    remote_user,
    remote_path,
    rsync_timeout,
    delete_after_copy,
    in_qbit,
    files_deleted
)
SELECT
    json_extract(value, '$.hash'),
    json_extract(value, '$.name'),
    json_extract(value, '$.status'),
    json_extract(value, '$.progress'),
    json_extract(value, '$.url'),
    json_extract(value, '$.save_path'),
    json_extract(value, '$.remote_host'),
    json_extract(value, '$.remote_user'),
    json_extract(value, '$.remote_path'),
    json_extract(value, '$.rsync_timeout'),
    json_extract(value, '$.delete_after_copy'),
    json_extract(value, '$.in_qbit'),
    json_extract(value, '$.files_deleted')
FROM json_each(readfile('state.json'));

COMMIT;
SQL
```
