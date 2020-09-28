use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset, Utc};

use crate::entities::{File, FileSort};
use crate::errors::*;
use crate::path::{self, AbsPath, IntoAbsPath, ScopedPath};
use crate::storage::{self, Storage};
use crate::tree::Tree;

#[derive(Debug, PartialEq)]
pub enum PathStatus {
    Missing,
    Modified,
    Tagged,
    Untagged,
}

pub struct StatusEntry {
    pub path: AbsPath,
    pub status: PathStatus,
}

pub struct Report {
    pub entries: Vec<StatusEntry>,
    paths: HashSet<AbsPath>,
}

impl Report {
    fn new() -> Self {
        Report {
            entries: vec![],
            paths: HashSet::new(),
        }
    }

    fn add_entry(&mut self, path: AbsPath, status: PathStatus) {
        self.entries.push(StatusEntry {
            path: path.clone(),
            status,
        });
        self.paths.insert(path);
    }

    fn contains_path(&self, path: &AbsPath) -> bool {
        self.paths.contains(path)
    }
}

pub fn database_status(db_path: &Path, recursive: bool) -> Result<Report> {
    info!("Retrieving all files from database");

    let mut store = Storage::open(&db_path)?;
    let root_path = store.root_path.clone();
    let mut tx = store.begin_transaction()?;

    let db_files = storage::file::files(&mut tx, FileSort::Name)?;

    let mut report = Report::new();

    check_files(&db_files, &root_path, &mut report)?;

    let mut tree = Tree::new();
    for db_file in db_files {
        tree.add(
            &db_file.to_path_buf().into_abs_path(&root_path),
            db_file.is_dir,
        );
    }

    let top_level_paths = tree.top_level().paths();
    for path in top_level_paths {
        find_new_files(AbsPath::from_unchecked(path), &mut report, recursive)?;
    }

    tx.commit()?;

    Ok(report)
}

pub fn files_status(
    db_path: &Path,
    paths: &[PathBuf],
    recursive: bool,
    follow_symlinks: bool,
) -> Result<Report> {
    let mut store = Storage::open(&db_path)?;
    let root_path = store.root_path.clone();
    let mut tx = store.begin_transaction()?;

    let mut report = Report::new();

    for path in paths {
        let abs_path = AbsPath::from(&path, &root_path);

        info!("{} resolving file", path.display());
        let resolved_path = path::resolve_path(&abs_path, follow_symlinks)?;
        let is_symlink = resolved_path == abs_path.to_path_buf();

        info!("{}: checking file in database", path.display());
        let scoped_path = ScopedPath::new(root_path.clone(), &resolved_path)?;
        let file_opt = storage::file::file_by_path(&mut tx, &scoped_path)?;
        if let Some(file) = file_opt {
            check_file(&abs_path, &file, &mut report)?;
        }

        if recursive && (follow_symlinks || !is_symlink) {
            info!("{}: retrieving files from database", path.display());

            let db_files = storage::file::files_by_directory(&mut tx, &scoped_path)?;
            check_files(&db_files, &root_path, &mut report)?;
        }

        find_new_files(abs_path, &mut report, recursive)?;
    }

    tx.commit()?;

    Ok(report)
}

fn check_files(files: &[File], root_path: &AbsPath, report: &mut Report) -> Result<()> {
    for file in files {
        let abs_path = file.to_path_buf().into_abs_path(root_path);
        check_file(&abs_path, file, report)?;
    }

    Ok(())
}

fn check_file(abs_path: &AbsPath, file: &File, report: &mut Report) -> Result<()> {
    info!("{}: checking file status", abs_path.display());

    if !abs_path.exists() {
        info!("{}: file is missing", abs_path.display());
        report.add_entry(abs_path.clone(), PathStatus::Missing);

        return Ok(());
    }

    let metadata = abs_path
        .metadata()
        .map_err(|e| format!("{}: could not stat: {}", abs_path.display(), e))?;
    let mod_time = metadata.modified()?;
    let mod_time_utc: DateTime<Utc> = mod_time.into();
    let mod_time_fixed: DateTime<FixedOffset> = mod_time_utc.into();

    if metadata.len() != file.size || file.mod_time != mod_time_fixed {
        info!("{}: file is modified", abs_path.display());
        report.add_entry(abs_path.clone(), PathStatus::Modified);
    } else {
        info!("{}: file is unchanged", abs_path.display());
        report.add_entry(abs_path.clone(), PathStatus::Tagged);
    }

    Ok(())
}

fn find_new_files(search_path: AbsPath, report: &mut Report, recursive: bool) -> Result<()> {
    info!("{}: finding new files", search_path.display());

    if !report.contains_path(&search_path) {
        report.add_entry(search_path.clone(), PathStatus::Untagged);
    }

    if recursive && search_path.is_dir() {
        // Sort directory entries
        let read_dir_iter = search_path.read_dir().map_err(|e| {
            format!(
                "{}: could not read directory listing: {}",
                search_path.display(),
                e
            )
        })?;
        let mut entries = vec![];
        for entry in read_dir_iter {
            entries.push(entry?.path());
        }
        entries.sort();

        for entry in entries {
            find_new_files(AbsPath::from_unchecked(entry), report, recursive)?;
        }
    }

    Ok(())
}
