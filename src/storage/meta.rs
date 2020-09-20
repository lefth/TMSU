//! This module contains functions which act across several tables (in separate queries) and
//! contain some business logic.
//! It is a tradeoff between having a full "storage" layer like in the Go implementation and mixing
//! responsibilities too much.
//!
//! Note that it might be cleaner to move this eventually to the "api" layer, e.g. in a "common" submodule.
use crate::entities::{FileId, FileTag, OptionalValueId, Tag, TagId, Value};
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

pub fn delete_file_tags_by_file_id(tx: &mut Transaction, file_id: &FileId) -> Result<()> {
    storage::filetag::delete_file_tags_by_file_id(tx, file_id)?;
    storage::meta::delete_file_if_untagged(tx, &file_id)
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

pub fn delete_file_tag(
    tx: &mut Transaction,
    file_id: &FileId,
    tag_id: &TagId,
    value_id: &OptionalValueId,
) -> Result<()> {
    storage::filetag::delete_file_tag(tx, file_id, tag_id, value_id)?;
    delete_file_if_untagged(tx, file_id)?;

    Ok(())
}

fn delete_file_if_untagged(tx: &mut Transaction, file_id: &FileId) -> Result<()> {
    let count = file_tag_count_by_file_id(tx, file_id, true)?;
    if count == 0 {
        storage::file::delete_file(tx, file_id)?;
    }

    Ok(())
}

fn file_tag_count_by_file_id(
    tx: &mut Transaction,
    file_id: &FileId,
    explicit_only: bool,
) -> Result<usize> {
    if explicit_only {
        // This differs slightly from the Go implementation, because we don't implement a
        // separate query for the count
        let file_tags = storage::filetag::file_tags_by_file_id(tx, file_id)?;
        return Ok(file_tags.len());
    }

    let file_tags = file_tags_by_file_id(tx, file_id, false)?;
    Ok(file_tags.len())
}

fn file_tags_by_file_id(
    tx: &mut Transaction,
    file_id: &FileId,
    explicit_only: bool,
) -> Result<Vec<FileTag>> {
    let mut file_tags = storage::filetag::file_tags_by_file_id(tx, file_id)?;

    if !explicit_only {
        file_tags = add_implied_file_tags(tx, file_tags)?;
    }
    Ok(file_tags)
}

pub fn add_implied_file_tags(
    tx: &mut Transaction,
    file_tags: Vec<FileTag>,
) -> Result<Vec<FileTag>> {
    let mut all_file_tags = file_tags.clone();

    let mut to_process = file_tags;
    while !to_process.is_empty() {
        let file_tag = to_process.pop().unwrap();

        let implications =
            storage::implication::implications_for(tx, &[file_tag.to_tag_id_value_id_pair()])?;

        for implication in implications.iter() {
            let existing_file_tag_opt = find_file_tag_for_pair(
                &mut all_file_tags,
                &implication.implied_tag.id,
                &implication.implied_value,
            );

            match existing_file_tag_opt {
                Some(file_tag) => file_tag.implicit = true,
                None => {
                    let new_file_tag = FileTag {
                        file_id: file_tag.file_id,
                        tag_id: implication.implied_tag.id,
                        value_id: OptionalValueId::from_opt_value(&implication.implied_value),
                        explicit: false,
                        implicit: true,
                    };
                    all_file_tags.push(new_file_tag.clone());
                    to_process.push(new_file_tag);
                }
            };
        }
    }

    Ok(all_file_tags)
}

fn find_file_tag_for_pair<'a>(
    file_tags: &'a mut Vec<FileTag>,
    tag_id: &TagId,
    opt_value: &Option<Value>,
) -> Option<&'a mut FileTag> {
    file_tags.iter_mut().find(|ft| {
        ft.tag_id == *tag_id && ft.value_id == OptionalValueId::from_opt_value(opt_value)
    })
}
