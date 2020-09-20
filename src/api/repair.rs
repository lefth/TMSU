use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use chrono::{DateTime, FixedOffset, Utc};

use crate::entities::{settings::Settings, File};
use crate::errors::*;
use crate::fingerprint;
use crate::path::{CanonicalPath, IntoAbsPath, ScopedPath};
use crate::storage::{self, Storage, Transaction};

pub fn manual_repair(
    db_path: &Path,
    from_path: &Path,
    to_path: &Path,
    pretend: bool,
) -> Result<()> {
    let mut store = Storage::open(&db_path)?;
    let root_path = store.root_path.clone();
    let mut tx = store.begin_transaction()?;

    let scoped_from_path = ScopedPath::new(root_path.clone(), from_path)?;
    let scoped_to_path = ScopedPath::new(root_path, to_path)?;

    info!("Loading settings");
    let settings = storage::setting::settings(&mut tx)?;

    info!(
        "Retrieving files under '{}' from the database",
        from_path.display()
    );

    let from_file_opt = storage::file::file_by_path(&mut tx, &scoped_from_path)
        .map_err(|e| format!("{}: could not retrieve file: {}", from_path.display(), e))?;

    if let Some(db_file) = from_file_opt {
        info!("{}: updating to {}", from_path.display(), to_path.display());

        if !pretend {
            manual_repair_file(&mut tx, &settings, &db_file, &scoped_to_path)?;
        }
    }

    let db_files = storage::file::files_by_directory(&mut tx, &scoped_from_path)
        .map_err(|e| format!("could not retrieve files from storage: {}", e))?;

    for db_file in db_files {
        info!("{}: updating to {}", from_path.display(), to_path.display());
        if !pretend {
            manual_repair_file(&mut tx, &settings, &db_file, &scoped_to_path)?;
        }
    }

    Ok(())
}

fn manual_repair_file(
    tx: &mut Transaction,
    settings: &Settings,
    db_file: &File,
    to_path: &ScopedPath,
) -> Result<()> {
    // Note: unlike the Go implementation, we don't check for permissions issues
    if !to_path.exists() {
        return Err(format!("{}: file not found", to_path.display()).into());
    }

    // Note: unlike the Go implementation, failing to create the fingerprint is fatal
    let fingerprint = fingerprint::create(
        to_path,
        &settings.file_fingerprint_algorithm()?,
        &settings.directory_fingerprint_algorithm()?,
        &settings.symlink_fingerprint_algorithm()?,
    )?;

    let metadata = to_path.metadata()?;
    let mod_time = metadata.modified()?;
    let mod_time_utc: DateTime<Utc> = mod_time.into();
    let size = metadata.len();
    let is_dir = metadata.is_dir();

    storage::file::update_file(
        tx,
        &db_file.id,
        to_path,
        fingerprint,
        mod_time_utc.into(),
        size,
        is_dir,
    )?;
    Ok(())
}

pub fn full_repair(
    db_path: &Path,
    search_paths: &[PathBuf],
    path: &Option<PathBuf>,
    remove_missing: bool,
    recalc_unmodified: bool,
    rationalize: bool,
    pretend: bool,
) -> Result<()> {
    let mut store = Storage::open(&db_path)?;
    let root_path = store.root_path.clone();
    let mut tx = store.begin_transaction()?;

    let settings = storage::setting::settings(&mut tx)?;

    let scoped_base_path = ScopedPath::new(
        root_path.clone(),
        path.as_ref().unwrap_or(&PathBuf::from("")),
    )?;

    info!(
        "Retrieving files under '{}' from the database",
        scoped_base_path.display()
    );
    let mut db_files = storage::file::files_by_directory(&mut tx, &scoped_base_path)
        .map_err(|e| format!("could not retrieve files from storage: {}", e))?;

    let db_file = storage::file::file_by_path(&mut tx, &scoped_base_path)
        .map_err(|e| format!("could not retrieve file from storage: {}", e))?;
    if let Some(db_file) = db_file {
        db_files.push(db_file);
    }

    info!(
        "Retrieved {} files from the database for path '{}'",
        db_files.len(),
        scoped_base_path.display()
    );

    let statuses = determine_statuses(&db_files, root_path.clone())?;

    if recalc_unmodified {
        repair_unmodified(
            &mut tx,
            &statuses.unmodified,
            root_path.clone(),
            pretend,
            &settings,
        )?;
    }

    repair_modified(
        &mut tx,
        &statuses.modified,
        root_path.clone(),
        pretend,
        &settings,
    )?;

    repair_moved(
        &mut tx,
        &statuses.missing,
        root_path.clone(),
        search_paths,
        pretend,
        &settings,
    )?;

    repair_missing(&mut tx, &statuses.missing, root_path, pretend, remove_missing)?;

    delete_untagged_files(&mut tx, &db_files)?;

    if rationalize {
        rationalize_file_tags(&mut tx, &db_files)?;
    }

    tx.commit()
}

fn repair_unmodified(
    tx: &mut Transaction,
    unmodified: &[&File],
    root_path: Rc<CanonicalPath>,
    pretend: bool,
    settings: &Settings,
) -> Result<()> {
    info!("Recalculating fingerprints for unmodified files");

    for db_file in unmodified {
        let scoped_path = ScopedPath::new(root_path.clone(), db_file.to_path_buf())?;

        let metadata = scoped_path.metadata()?;
        let mod_time = metadata.modified()?;
        let mod_time_utc: DateTime<Utc> = mod_time.into();

        // Note: unlike the Go implementation, failing to create the fingerprint is fatal
        let fingerprint = fingerprint::create(
            &scoped_path,
            &settings.file_fingerprint_algorithm()?,
            &settings.directory_fingerprint_algorithm()?,
            &settings.symlink_fingerprint_algorithm()?,
        )?;

        if !pretend {
            storage::file::update_file(
                tx,
                &db_file.id,
                &scoped_path,
                fingerprint,
                mod_time_utc.into(),
                metadata.len(),
                metadata.is_dir(),
            )
            .map_err(|e| {
                format!(
                    "{}: could not update file in database: {}",
                    scoped_path.display(),
                    e
                )
            })?;
        }

        println!("{}: recalculated fingerprint", scoped_path.display())
    }

    Ok(())
}

fn repair_modified(
    tx: &mut Transaction,
    modified: &[&File],
    root_path: Rc<CanonicalPath>,
    pretend: bool,
    settings: &Settings,
) -> Result<()> {
    info!("Repairing modified files");

    for db_file in modified {
        let scoped_path = ScopedPath::new(root_path.clone(), db_file.to_path_buf())?;

        let metadata = scoped_path.metadata()?;
        let mod_time = metadata.modified()?;
        let mod_time_utc: DateTime<Utc> = mod_time.into();

        // Note: unlike the Go implementation, failing to create the fingerprint is fatal
        let fingerprint = fingerprint::create(
            &scoped_path,
            &settings.file_fingerprint_algorithm()?,
            &settings.directory_fingerprint_algorithm()?,
            &settings.symlink_fingerprint_algorithm()?,
        )?;

        if !pretend {
            storage::file::update_file(
                tx,
                &db_file.id,
                &scoped_path,
                fingerprint,
                mod_time_utc.into(),
                metadata.len(),
                metadata.is_dir(),
            )
            .map_err(|e| {
                format!(
                    "{}: could not update file in database: {}",
                    scoped_path.display(),
                    e
                )
            })?;
        }

        // FIXME: should be done in the CLI layer instead
        println!("{}: updated fingerprint", scoped_path.display());
    }

    Ok(())
}

fn repair_moved(
    tx: &mut Transaction,
    missing: &[&File],
    root_path: Rc<CanonicalPath>,
    search_paths: &[PathBuf],
    pretend: bool,
    settings: &Settings,
) -> Result<()> {
    info!("Repairing moved files");

    // Don't bother enumerating filesystem if nothing to do
    if missing.is_empty() || search_paths.is_empty() {
        return Ok(());
    }

    let paths_by_size = build_paths_by_size_map(search_paths)?;

    for db_file in missing {
        let abs_db_file = db_file.to_path_buf().into_abs_path(&*root_path);
        debug!("{}: searching for new location", abs_db_file.display());

        if let Some(paths_of_size) = paths_by_size.get(&db_file.size) {
            info!(
                "{}: file is of size {}, identified {} files of this size",
                abs_db_file.display(),
                db_file.size,
                paths_of_size.len()
            );

            for candidate_path in paths_of_size {
                let scoped_candidate = ScopedPath::new(root_path.clone(), &candidate_path)?;
                let candidate_file = storage::file::file_by_path(tx, &scoped_candidate)?;
                if candidate_file.is_some() {
                    // The file is already tagged
                    continue;
                }

                let metadata = candidate_path.metadata()?;
                let mod_time = metadata.modified()?;
                let mod_time_utc: DateTime<Utc> = mod_time.into();

                let fingerprint = fingerprint::create(
                    &candidate_path,
                    &settings.file_fingerprint_algorithm()?,
                    &settings.directory_fingerprint_algorithm()?,
                    &settings.symlink_fingerprint_algorithm()?,
                )?;

                if fingerprint == db_file.fingerprint {
                    if !pretend {
                        storage::file::update_file(
                            tx,
                            &db_file.id,
                            &scoped_candidate,
                            fingerprint,
                            mod_time_utc.into(),
                            db_file.size,
                            db_file.is_dir,
                        )?;
                    }

                    println!(
                        "{}: updated path to {}",
                        abs_db_file.display(),
                        candidate_path.display()
                    );

                    break;
                }
            }
        }
    }

    Ok(())
}

fn repair_missing(
    tx: &mut Transaction,
    missing: &[&File],
    root_path: Rc<CanonicalPath>,
    pretend: bool,
    force: bool,
) -> Result<()> {
    info!("Repairing missing files");

    for db_file in missing {
        let scoped_path = ScopedPath::new(root_path.clone(), db_file.to_path_buf())?;
        if force {
            if !pretend {
                storage::meta::delete_file_tags_by_file_id(tx, &db_file.id).map_err(|e| {
                    format!(
                        "{}: could not delete file-tags: {}",
                        scoped_path.display(),
                        e
                    )
                })?;
            }
            println!("{}: removed", scoped_path.display());
        } else {
            println!("{}: missing", scoped_path.display());
        }
    }

    Ok(())
}

fn delete_untagged_files(tx: &mut Transaction, db_files: &[File]) -> Result<()> {
    info!("Purging untagged files");

    let file_ids: Vec<_> = db_files.iter().map(|f| f.id).collect();
    storage::file::delete_untagged_files(tx, &file_ids)?;

    Ok(())
}

fn rationalize_file_tags(tx: &mut Transaction, db_files: &[File]) -> Result<()> {
    info!("Rationalizing file tags");

    for file in db_files {
        let file_tags = storage::filetag::file_tags_by_file_id(tx, &file.id).map_err(|e| {
            format!(
                "could not determine tags for file '{}': {}",
                file.to_path_buf().display(),
                e
            )
        })?;
        for file_tag in file_tags {
            if file_tag.explicit && file_tag.implicit {
                info!(
                    "{}: removing explicit tagging {} as implicit tagging exists",
                    file.to_path_buf().display(),
                    file_tag.file_id
                );
                storage::meta::delete_file_tag(
                    tx,
                    &file_tag.file_id,
                    &file_tag.tag_id,
                    &file_tag.value_id,
                )
                // FIXME: do not use {:?}
                .map_err(|e| {
                    format!(
                        "could not delete file tag for file {}, tag {} and value {:?}: {}",
                        &file_tag.file_id, &file_tag.tag_id, &file_tag.value_id, e
                    )
                })?;
            }
        }
    }

    Ok(())
}

struct Statuses<'a> {
    unmodified: Vec<&'a File>,
    modified: Vec<&'a File>,
    missing: Vec<&'a File>,
}

fn determine_statuses(db_files: &[File], root_path: Rc<CanonicalPath>) -> Result<Statuses> {
    info!("Determining file statuses");

    let mut modified = vec![];
    let mut unmodified = vec![];
    let mut missing = vec![];

    for db_file in db_files.iter() {
        let abs_path = db_file.to_path_buf().into_abs_path(&*root_path);
        // Note: unlike the Go implementation, we don't check for permissions issues
        if !abs_path.exists() {
            info!("{}: missing", abs_path.display());
            missing.push(db_file);
            continue;
        }

        let metadata = abs_path.metadata()?;
        let mod_time = metadata.modified()?;
        let mod_time_utc: DateTime<Utc> = mod_time.into();
        let mod_time_fixed: DateTime<FixedOffset> = mod_time_utc.into();

        if db_file.size == metadata.len() && db_file.mod_time == mod_time_fixed {
            debug!("{}: unmodified", abs_path.display());
            unmodified.push(db_file);
        } else {
            debug!("{}: modified", abs_path.display());
            modified.push(db_file);
        }
    }

    Ok(Statuses {
        modified,
        unmodified,
        missing,
    })
}

fn build_paths_by_size_map(paths: &[PathBuf]) -> Result<HashMap<u64, Vec<PathBuf>>> {
    debug!("Building map of paths by size");

    let mut paths_by_size = HashMap::new();

    for path in paths {
        build_paths_by_size_map_recursive(path.clone(), &mut paths_by_size)?;
    }

    debug!("The path by size map has {} entries", paths_by_size.len());

    Ok(paths_by_size)
}

fn build_paths_by_size_map_recursive(
    path: PathBuf,
    paths_by_size: &mut HashMap<u64, Vec<PathBuf>>,
) -> Result<()> {
    // Note: unlike the Go implementation, we don't check and warn for permissions issues
    let metadata = path.metadata()?;

    if metadata.is_dir() {
        debug!("{}: examining directory content", path.display());

        for entry in path.read_dir()? {
            let child_path = entry?.path();
            build_paths_by_size_map_recursive(child_path, paths_by_size)?;
        }
    } else {
        debug!("{}: file is of size {}", path.display(), metadata.len());

        if let Some(files_of_size) = paths_by_size.get_mut(&metadata.len()) {
            files_of_size.push(path);
        } else {
            paths_by_size.insert(metadata.len(), vec![path]);
        }
    }

    Ok(())
}
