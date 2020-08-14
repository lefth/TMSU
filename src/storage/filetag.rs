use crate::entities::{FileId, FileTag, OptionalValueId, TagId, ValueId};
use crate::errors::*;
use crate::storage::{Row, Transaction};

pub fn file_tag_count(tx: &mut Transaction) -> Result<u64> {
    tx.count_from_table("file_tag")
}

pub fn file_tags_by_tag_id(tx: &mut Transaction, tag_id: &TagId) -> Result<Vec<FileTag>> {
    let sql = "
SELECT file_id, tag_id, value_id
FROM file_tag
WHERE tag_id = ?1";

    let params = rusqlite::params![tag_id];
    tx.query_vec_params(sql, params, parse_file_tag)
}

pub fn file_tags_by_value_id(tx: &mut Transaction, value_id: &ValueId) -> Result<Vec<FileTag>> {
    let sql = "
SELECT file_id, tag_id, value_id
FROM file_tag
WHERE value_id = ?1";

    let params = rusqlite::params![value_id];
    tx.query_vec_params(sql, params, parse_file_tag)
}

pub fn file_tags_by_file_id(tx: &mut Transaction, file_id: &FileId) -> Result<Vec<FileTag>> {
    let sql = "
SELECT file_id, tag_id, value_id
FROM file_tag
WHERE file_id = ?1";

    let params = rusqlite::params![file_id];
    tx.query_vec_params(sql, params, parse_file_tag)
}

fn parse_file_tag(row: Row) -> Result<FileTag> {
    Ok(FileTag {
        file_id: row.get(0)?,
        tag_id: row.get(1)?,
        value_id: OptionalValueId::from_id(row.get(2)?),
        explicit: true,
        implicit: false,
    })
}

pub fn add_file_tag(
    tx: &mut Transaction,
    file_id: &FileId,
    tag_id: &TagId,
    value_id: OptionalValueId,
) -> Result<usize> {
    let sql = "
INSERT OR IGNORE INTO file_tag (file_id, tag_id, value_id)
VALUES (?1, ?2, ?3)";

    let params = rusqlite::params![file_id, tag_id, value_id];
    tx.execute_params(sql, params)
}

pub fn delete_file_tags_by_tag_id(tx: &mut Transaction, tag_id: &TagId) -> Result<usize> {
    let sql = "
DELETE FROM file_tag
WHERE tag_id = ?";

    let params = rusqlite::params![tag_id];
    tx.execute_params(sql, params)
}

pub fn delete_file_tags_by_value_id(tx: &mut Transaction, value_id: &ValueId) -> Result<usize> {
    let sql = "
DELETE FROM file_tag
WHERE value_id = ?";

    let params = rusqlite::params![value_id];
    tx.execute_params(sql, params)
}

pub fn copy_file_tags(
    tx: &mut Transaction,
    src_tag_id: &TagId,
    dest_tag_id: &TagId,
) -> Result<usize> {
    let sql = "
INSERT INTO file_tag (file_id, tag_id, value_id)
SELECT file_id, ?2, value_id
FROM file_tag
WHERE tag_id = ?1";

    let params = rusqlite::params![src_tag_id, dest_tag_id];
    tx.execute_params(sql, params)
}
