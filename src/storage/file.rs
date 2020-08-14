use chrono::DateTime;

use crate::entities::{File, FileId};
use crate::errors::*;
use crate::path::ScopedPath;
use crate::storage::{path_to_sql, Row, Transaction};

const TIMESTAMP_FORMAT: &str = "%F %T%.f%:z";

pub fn file_count(tx: &mut Transaction) -> Result<u64> {
    tx.count_from_table("file")
}

pub fn file_by_path(tx: &mut Transaction, scoped_path: &ScopedPath) -> Result<Option<File>> {
    let sql = "
SELECT id, directory, name, fingerprint, mod_time, size, is_dir
FROM file
WHERE directory = ? AND name = ?";

    let (dir, name) = scoped_path.inner_as_dir_and_name();

    let params = rusqlite::params![path_to_sql(dir)?, path_to_sql(name)?];
    tx.query_single_params(sql, params, parse_file)
}

fn parse_file(row: Row) -> Result<File> {
    let mod_time_str: String = row.get(4)?;
    let mod_time = DateTime::parse_from_str(&mod_time_str, TIMESTAMP_FORMAT)?;

    Ok(File {
        id: row.get(0)?,
        dir: row.get(1)?,
        name: row.get(2)?,
        fingerprint: row.get(3)?,
        mod_time,
        size: row.get_u64(5)?,
        is_dir: row.get(6)?,
    })
}

pub fn delete_untagged_files(tx: &mut Transaction, file_ids: &[FileId]) -> Result<()> {
    let sql = "
DELETE FROM file
WHERE id = ?1
AND (SELECT count(1)
     FROM file_tag
     WHERE file_id = ?1) == 0";

    for file_id in file_ids {
        let params = rusqlite::params![file_id];
        tx.execute_params(sql, params)?;
    }

    Ok(())
}
