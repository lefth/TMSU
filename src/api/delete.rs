use std::path::Path;

use crate::api;
use crate::errors::*;
use crate::storage::{self, Storage};

pub fn run_delete_tag(db_path: &Path, tag_names: &[&str]) -> Result<()> {
    let mut store = Storage::open(&db_path)?;
    let mut tx = store.begin_transaction()?;

    for name in tag_names {
        let tag = api::load_existing_tag(&mut tx, name)?;

        info!("Deleting tag '{}'", name);

        storage::meta::delete_tag(&mut tx, &tag)
            .map_err(|e| format!("could not delete tag '{}': {}", name, e))?;
    }

    tx.commit()
}

pub fn run_delete_value(db_path: &Path, value_names: &[&str]) -> Result<()> {
    let mut store = Storage::open(&db_path)?;
    let mut tx = store.begin_transaction()?;

    for name in value_names {
        let value = api::load_existing_value(&mut tx, name)?;

        info!("Deleting value '{}'", name);

        storage::meta::delete_value(&mut tx, &value)
            .map_err(|e| format!("could not delete value '{}': {}", name, e))?;
    }

    tx.commit()
}
