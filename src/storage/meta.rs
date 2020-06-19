//! This module contains functions which act across several tables (in separate queries) and
//! contain some business logic.
//! It is a tradeoff between having a full "storage" layer like in the Go implementation and mixing
//! responsibilities too much.
//!
//! Note that it might be cleaner to move this eventually to the "api" layer, e.g. in a "common" submodule.
use crate::entities::{FileId, FileTag, Tag, Value};
use crate::errors::*;
use crate::storage::{self, Transaction};

pub fn delete_tag(tx: &mut Transaction, tag: &Tag) -> Result<()> {
    delete_file_tags_by_tag_id(tx, tag)?;
    storage::implication::delete_implications_by_tag_id(tx, &tag.id)?;
    storage::tag::delete_tag(tx, &tag.id)
}

pub fn delete_value(tx: &mut Transaction, value: &Value) -> Result<()> {
    delete_file_tags_by_value_id(tx, value)?;
    storage::implication::delete_implications_by_value_id(tx, &value.id)?;
    storage::value::delete_value(tx, &value.id)
}

fn delete_file_tags_by_tag_id(tx: &mut Transaction, tag: &Tag) -> Result<()> {
    let file_tags = storage::filetag::file_tags_by_tag_id(tx, &tag.id)?;
    storage::filetag::delete_file_tags_by_tag_id(tx, &tag.id)?;
    let file_ids = extract_file_ids(&file_tags);
    storage::file::delete_untagged_files(tx, &file_ids)
}

fn delete_file_tags_by_value_id(tx: &mut Transaction, value: &Value) -> Result<()> {
    let file_tags = storage::filetag::file_tags_by_value_id(tx, &value.id)?;
    storage::filetag::delete_file_tags_by_value_id(tx, &value.id)?;
    let file_ids = extract_file_ids(&file_tags);
    storage::file::delete_untagged_files(tx, &file_ids)
}

fn extract_file_ids(file_tags: &[FileTag]) -> Vec<FileId> {
    file_tags.iter().map(|ft| ft.file_id).collect()
}
