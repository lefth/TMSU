use std::path::PathBuf;

use lazy_static::lazy_static;
use structopt::StructOpt;

use crate::api;
use crate::cli::{locate_db, GlobalOptions};
use crate::errors::*;

lazy_static! {
    static ref EXAMPLES: String = super::generate_examples(&[
        ("tmsu repair", None),
        ("tmsu repair /new/path # look for missing files here", None),
        (
            "tmsu repair --path=/home/sally # repair subset of database",
            None
        ),
        (
            "tmsu repair --manual /home/bob /home/fred # manually repair paths",
            None
        ),
    ]);
}

/// Fixes broken paths and stale fingerprints in the database caused by file modifications and
/// moves.
///
/// Modified files are identified by a change to the file's modification time or file size. These
/// files are repaired by updating the details in the database.
///
/// An attempt is made to find missing files under PATHs specified. If a file with the same
/// fingerprint is found then the database is updated with the new file's details. If no PATHs are
/// specified, or no match can be found, then the file is instead reported as missing.
///
/// Files that have been both moved and modified cannot be repaired and must be manually relocated.
///
/// When run with the --manual option, any paths that begin with OLD are updated to begin with NEW.
/// Any affected files' fingerprints are updated providing the file exists at the new location. No
/// further repairs are attempted in this mode.
#[derive(Debug, StructOpt)]
#[structopt(after_help(EXAMPLES.as_str()))]
pub struct RepairOptions {
    /// Limit repair to files in database under PATH
    #[structopt(name("path"), short, long)]
    base_path: Option<PathBuf>,

    /// Do not make any changes
    #[structopt(short("P"), long)]
    pretend: bool,

    /// Remove missing files from the database
    #[structopt(short("R"), long("remove"))]
    remove_missing: bool,

    /// Remove explicit taggings where an implicit tagging exists
    #[structopt(long)]
    rationalize: bool,

    /// Recalculate fingerprints for unmodified files
    #[structopt(short, long)]
    unmodified: bool,

    /// Manually relocate files
    #[structopt(short, long, conflicts_with_all(&["remove", "rationalize", "unmodified"]))]
    manual: bool,

    /// File paths
    #[structopt(conflicts_with("values"))]
    paths: Vec<PathBuf>,
}

impl RepairOptions {
    pub fn execute(&self, global_opts: &GlobalOptions) -> Result<()> {
        let db_path = locate_db(&global_opts.database)?;
        info!("Database path: {}", db_path.display());

        if self.manual {
            if self.paths.len() != 2 {
                return Err("Expected two arguments for the --manual option".into());
            }

            api::repair::manual_repair(&db_path, &self.paths[0], &self.paths[1], self.pretend)?;
        } else {
            api::repair::full_repair(
                &db_path,
                &self.paths,
                &self.base_path,
                self.remove_missing,
                self.unmodified,
                self.rationalize,
                self.pretend,
            )?;
        }

        Ok(())
    }
}
