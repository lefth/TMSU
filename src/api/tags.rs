use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::api;
use crate::entities::{FileId, ValueId};
use crate::errors::*;
use crate::path::{self, ScopedPath};
use crate::storage::{self, Storage, Transaction};

/// One group of tags. If the value name is present, then the tags correspond to it
#[derive(Debug)]
pub struct ValueTagGroup {
    pub value_name: Option<String>,
    pub tag_names: Vec<String>,
}

#[derive(Debug)]
pub struct FileTagGroup {
    pub path: PathBuf,
    pub tags: Vec<TagData>,
}

#[derive(Debug)]
pub struct TagData {
    pub tag_name: String,
    pub value_name: Option<String>,
    pub explicit: bool,
    pub implicit: bool,
}

pub fn list_all_tags(db_path: &Path) -> Result<Vec<ValueTagGroup>> {
    let mut store = Storage::open(&db_path)?;
    let mut tx = store.begin_transaction()?;

    info!("Retrieving all tags");

    let tag_names = storage::tag::tags(&mut tx)?
        .iter()
        .map(|v| v.name.to_owned())
        .collect();

    tx.commit()?;

    Ok(vec![ValueTagGroup {
        value_name: None,
        tag_names,
    }])
}

/// If `value_names` is empty, then all the tags are returned in one `ValueTagGroup` and this
/// function is equivalent to `list_all_tags`.
/// Otherwise, the output contains one `ValueTagGroup` per value name.
pub fn list_tags_for_values(db_path: &Path, value_names: &[&str]) -> Result<Vec<ValueTagGroup>> {
    if value_names.is_empty() {
        Ok(list_all_tags(db_path)?)
    } else {
        let mut store = Storage::open(&db_path)?;
        let mut tx = store.begin_transaction()?;

        let mut tag_groups = Vec::with_capacity(value_names.len());

        for value_name in value_names {
            info!("Looking up value '{}'", value_name);
            let value = api::load_existing_value(&mut tx, value_name)?;

            info!("Retrieving tags for value '{}'", value_name);
            let tag_names = tag_names_by_value_id(&mut tx, &value.id)?;

            tag_groups.push(ValueTagGroup {
                value_name: Some((*value_name).to_owned()),
                tag_names,
            });
        }

        tx.commit()?;

        Ok(tag_groups)
    }
}

fn tag_names_by_value_id(tx: &mut Transaction, value_id: &ValueId) -> Result<Vec<String>> {
    let file_tags = storage::filetag::file_tags_by_value_id(tx, value_id)?;

    let mut tag_names = HashSet::new();

    for file_tag in file_tags {
        let tag_opt = storage::tag::tag_by_id(tx, &file_tag.tag_id)?;

        match tag_opt {
            Some(tag) => tag_names.insert(tag.name),
            None => return Err(format!("tag '{}' does not exist", file_tag.tag_id).into()),
        };
    }

    let mut names_as_vec: Vec<_> = tag_names.into_iter().collect();
    names_as_vec.sort();

    Ok(names_as_vec)
}

pub fn list_tags_for_paths(
    db_path: &Path,
    paths: &[PathBuf],
    follow_symlinks: bool,
    explicit_only: bool,
) -> Result<Vec<FileTagGroup>> {
    let mut store = Storage::open(&db_path)?;
    let root_path = store.root_path.clone();
    let mut tx = store.begin_transaction()?;

    let mut tag_groups = Vec::with_capacity(paths.len());

    for path in paths {
        info!("Resolving path '{}'", path.display());
        // Get metadata without following symlinks
        let resolved_path = path::resolve_path(path, follow_symlinks)?;

        info!("Looking up file '{}'", resolved_path.display());
        let scoped_path = ScopedPath::new(root_path.clone(), &resolved_path)?;
        let file_opt = storage::file::file_by_path(&mut tx, &scoped_path)?;

        info!("Retrieving tags");
        if let Some(file) = file_opt {
            let tags = tag_data_by_file_id(&mut tx, &file.id, explicit_only)?;

            // Make sure to return the input path
            tag_groups.push(FileTagGroup {
                path: path.clone(),
                tags,
            });
        }
    }

    tx.commit()?;

    Ok(tag_groups)
}

fn tag_data_by_file_id(
    tx: &mut Transaction,
    file_id: &FileId,
    explicit_only: bool,
) -> Result<Vec<TagData>> {
    // Get explicit file tags
    let mut file_tags = storage::filetag::file_tags_by_file_id(tx, file_id)?;

    // Add implicit (implied) file tags
    if !explicit_only {
        file_tags = storage::meta::add_implied_file_tags(tx, file_tags)?;
    }

    let mut tag_data = Vec::with_capacity(file_tags.len());

    for file_tag in file_tags {
        let tag_opt = storage::tag::tag_by_id(tx, &file_tag.tag_id)?;

        let tag = match tag_opt {
            Some(tag) => tag,
            None => return Err(format!("tag '{}' does not exist", file_tag.tag_id).into()),
        };

        let mut value_name_opt = None;
        if let Some(value_id) = *file_tag.value_id {
            let value_opt = storage::value::value_by_id(tx, &value_id)?;

            match value_opt {
                Some(value) => value_name_opt = Some(value.name),
                None => return Err(format!("value '{}' does not exist", value_id).into()),
            };
        }

        tag_data.push(TagData {
            tag_name: tag.name,
            value_name: value_name_opt,
            explicit: file_tag.explicit,
            implicit: file_tag.implicit,
        });
    }

    tag_data.sort_by(|a, b| a.tag_name.cmp(&b.tag_name));

    Ok(tag_data)
}
