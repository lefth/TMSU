use std::path::PathBuf;

use lazy_static::lazy_static;
use structopt::StructOpt;

use crate::api;
use crate::api::status::{PathStatus, Report};
use crate::cli::{locate_db, GlobalOptions};
use crate::errors::*;
use crate::path::AbsPath;

lazy_static! {
    static ref EXAMPLES: String = super::generate_examples(&[
        ("tmsu status", None),
        ("tmsu status .", None),
        ("tmsu status --directory *", None),
    ]);
}

/// Shows the status of PATHs.
///
/// Where PATHs are not specified the status of the database is shown.
///
///   T - Tagged
///   M - Modified
///   ! - Missing
///   U - Untagged
///
/// Status codes of T, M and ! mean that the file has been tagged (and thus is in the TMSU
/// database). Modified files are those with a different modification time or size to that in the
/// database. Missing files are those in the database but that no longer exist in the file-system.
///
/// Note: The repair subcommand can be used to fix problems caused by files that have been modified or moved on disk.
#[derive(Debug, StructOpt)]
#[structopt(after_help(EXAMPLES.as_str()))]
pub struct StatusOptions {
    /// Do not examine directory contents (non-recursive)
    #[structopt(short, long("directory"))]
    directory_only: bool,

    /// Do not follow symbolic links
    #[structopt(short("P"), long)]
    no_dereference: bool,

    /// File paths
    #[structopt()]
    paths: Vec<PathBuf>,
}

impl StatusOptions {
    pub fn execute(&self, global_opts: &GlobalOptions) -> Result<()> {
        let db_path = locate_db(&global_opts.database)?;
        info!("Database path: {}", db_path.display());

        let report = if self.paths.is_empty() {
            api::status::database_status(&db_path, !self.directory_only)?
        } else {
            api::status::files_status(
                &db_path,
                &self.paths,
                !self.directory_only,
                !self.no_dereference,
            )?
        };

        let cwd = super::getcwd()?;
        print_for_status(&report, &cwd, PathStatus::Tagged);
        print_for_status(&report, &cwd, PathStatus::Modified);
        print_for_status(&report, &cwd, PathStatus::Missing);
        print_for_status(&report, &cwd, PathStatus::Untagged);

        Ok(())
    }
}

fn print_for_status(report: &Report, base_path: &AbsPath, status: PathStatus) {
    for entry in &report.entries {
        let formatted_status = format_status(&status);
        if entry.status == status {
            let rel_path = super::rel_to(&entry.path, &base_path);
            println!("{} {}", formatted_status, rel_path.display());
        }
    }
}

fn format_status(status: &PathStatus) -> &str {
    match status {
        PathStatus::Untagged => "U",
        PathStatus::Tagged => "T",
        PathStatus::Modified => "M",
        PathStatus::Missing => "!",
    }
}
