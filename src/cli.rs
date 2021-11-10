mod config;
mod copy;
mod delete;
mod files;
mod imply;
mod info;
mod init;
mod merge;
mod rename;
mod repair;
mod status;
mod tags;
mod untagged;
mod values;

use std::env;
use std::path::PathBuf;
use std::process;
use std::result;
use std::str;

use ansi_term::Colour;
use structopt::clap::arg_enum;
use structopt::clap::AppSettings::{ColoredHelp, UnifiedHelpMessage};
use structopt::StructOpt;

use crate::errors::*;
use crate::path::AbsPath;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "TMSU",
    about = "A tool for tagging your files and accessing them through a virtual filesystem",
    global_setting(UnifiedHelpMessage),  // Merge options and flags in the usage output
    global_setting(ColoredHelp),  // Use colors by default
)]
pub struct TmsuOptions {
    // Externalize global options to a separate struct for convenience
    #[structopt(flatten)]
    pub global_opts: GlobalOptions,

    #[structopt(subcommand)]
    cmd: SubCommands,
}

#[derive(Debug, StructOpt)]
pub struct GlobalOptions {
    /// Use the specified database
    #[structopt(short = "-D", long, env = "TMSU_DB", parse(from_os_str))]
    database: Option<PathBuf>,

    /// Colorize the output (auto/always/never)
    #[structopt(long, default_value = "auto")]
    color: ColorMode,

    // public because this must be processed very early:
    #[structopt(long, short)]
    pub verbose: bool,
}

arg_enum! {
    #[derive(Debug)]
    enum ColorMode {
        Auto,
        Always,
        Never,
    }
}

#[derive(Debug, StructOpt)]
enum SubCommands {
    Config(config::ConfigOptions),
    Copy(copy::CopyOptions),
    Delete(delete::DeleteOptions),
    Files(files::FilesOptions),
    Imply(imply::ImplyOptions),
    Info(info::InfoOptions),
    Init(init::InitOptions),
    Merge(merge::MergeOptions),
    Rename(rename::RenameOptions),
    Repair(repair::RepairOptions),
    Status(status::StatusOptions),
    Tags(tags::TagsOptions),
    Untagged(untagged::UntaggedOptions),
    Values(values::ValuesOptions),
}

/// CLI entry point, dispatching to subcommands
pub fn run(opt: TmsuOptions) -> Result<()> {
    match opt.cmd {
        SubCommands::Config(config_opts) => config_opts.execute(&opt.global_opts),
        SubCommands::Copy(copy_opts) => copy_opts.execute(&opt.global_opts),
        SubCommands::Delete(delete_opts) => delete_opts.execute(&opt.global_opts),
        SubCommands::Files(files_opts) => files_opts.execute(&opt.global_opts),
        SubCommands::Imply(imply_opts) => imply_opts.execute(&opt.global_opts),
        SubCommands::Info(info_opts) => info_opts.execute(&opt.global_opts),
        SubCommands::Init(init_opts) => init_opts.execute(),
        SubCommands::Merge(merge_opts) => merge_opts.execute(&opt.global_opts),
        SubCommands::Rename(rename_opts) => rename_opts.execute(&opt.global_opts),
        SubCommands::Repair(repair_opts) => repair_opts.execute(&opt.global_opts),
        SubCommands::Status(status_opts) => status_opts.execute(&opt.global_opts),
        SubCommands::Tags(tags_opts) => tags_opts.execute(&opt.global_opts),
        SubCommands::Untagged(untagged_opts) => untagged_opts.execute(&opt.global_opts),
        SubCommands::Values(values_opts) => values_opts.execute(&opt.global_opts),
    }
}

fn locate_db(db_path: &Option<PathBuf>) -> Result<PathBuf> {
    // Use the given path if available
    match db_path {
        Some(path) => Ok(path.clone()),
        // Fallback: look for the DB in parent directories
        None => match find_database_upwards()? {
            Some(path) => Ok(path),
            // Fallback: use the default database
            None => match get_user_default_db() {
                Some(path) => Ok(path),
                // OK, we finally give up...
                None => Err(ErrorKind::NoDatabaseFound(PathBuf::default()).into()),
            },
        },
    }
}

/// Look for .tmsu/db in the current directory and ancestors
fn find_database_upwards() -> Result<Option<PathBuf>> {
    let mut path = env::current_dir()?;

    loop {
        let mut db_path = path.clone();
        db_path.push(".tmsu");
        db_path.push("db");

        debug!("Looking for database at {:?}", &db_path);
        if db_path.is_file() {
            return Ok(Some(db_path));
        }

        match path.parent() {
            Some(parent) => {
                path = PathBuf::from(parent);
            }
            None => {
                return Ok(None);
            }
        }
    }
}

/// Return the path corresponding to $HOME/.tmsu/default.db,
/// or None if the home directory cannot be resolved
fn get_user_default_db() -> Option<PathBuf> {
    dirs::home_dir().map(|mut path| {
        path.push(".tmsu");
        path.push("default.db");
        path
    })
}

fn is_stdout_tty() -> bool {
    atty::is(atty::Stream::Stdout)
}

fn should_use_colour(color_mode: &ColorMode) -> bool {
    match color_mode {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => is_stdout_tty(),
    }
}

pub fn print_error(result: Result<()>) {
    if let Err(error) = result {
        eprintln!("tmsu: {}", error);

        if let Some(backtrace) = error.backtrace() {
            eprintln!("backtrace: {:?}", backtrace);
        }

        process::exit(1);
    }
}

fn getcwd() -> Result<AbsPath> {
    Ok(AbsPath::from_unchecked(env::current_dir()?))
}

fn rel_to(path: &AbsPath, base: &AbsPath) -> PathBuf {
    if path == base {
        return PathBuf::from(".");
    }

    match path.strip_prefix(base) {
        // XXX: the extra "./" prefix is copied from the Go implementation. Should we get rid of it?
        Ok(p) => PathBuf::from(".").join(p),
        // TODO: get rid of this special handling?
        Err(_) => match base.parent() {
            None => path.to_path_buf(),
            Some(base_parent) => {
                if path.to_path_buf() == base_parent {
                    return PathBuf::from("..");
                }

                match path.strip_prefix(base_parent) {
                    // XXX: the extra "../" prefix is copied from the Go implementation. Should we
                    // get rid of it?
                    Ok(p) => PathBuf::from("..").join(p),
                    Err(_) => path.to_path_buf(),
                }
            }
        },
    }
}

fn generate_examples(examples: &[(&str, Option<&str>)]) -> String {
    // Simple indirection to make testing easier
    generate_examples_inner(is_stdout_tty(), examples)
}

fn generate_examples_inner(use_color: bool, examples: &[(&str, Option<&str>)]) -> String {
    // Define styles
    let header_style;
    let prompt_style;
    let comment_style;
    if use_color {
        header_style = Colour::Yellow.normal();
        prompt_style = Colour::Green.normal();
        comment_style = Colour::Cyan.normal();
    } else {
        header_style = ansi_term::Style::default();
        prompt_style = ansi_term::Style::default();
        comment_style = ansi_term::Style::default();
    }

    let prompt = prompt_style.paint("$");

    let formatted: Vec<_> = examples
        .iter()
        .map(|(cmd_line, output)| {
            // Handle comments
            let cmd_line = match cmd_line.find('#') {
                None => cmd_line.to_string(),
                Some(index) => format!(
                    "{}{}",
                    &cmd_line[..index],
                    comment_style.paint(&cmd_line[index..])
                ),
            };
            // Handle output
            let output_str = match output {
                Some(s) => format!("\n    {}", s.replace("\n", "\n    ")),
                None => "".to_string(),
            };
            format!("    {} {}{}", prompt, cmd_line, output_str)
        })
        .collect();
    format!(
        "{}\n{}",
        header_style.paint("EXAMPLES:"),
        formatted.join("\n")
    )
}

fn print_columns(strings: &[String]) {
    // TODO: do proper column printing
    if is_stdout_tty() {
        println!("{}", strings.join("  "));
    } else {
        for s in strings {
            println!("{}", s);
        }
    }
}

fn escape(string: String, chars: &[char]) -> String {
    let mut res = string;
    for chr in chars {
        res = res.replace(*chr, &format!("\\{}", chr));
    }
    res
}

#[derive(Debug)]
struct TagOrValueName {
    name: String,
}

// Allow automatic parsing in StructOpt
impl str::FromStr for TagOrValueName {
    type Err = Error;

    fn from_str(raw: &str) -> result::Result<Self, Self::Err> {
        if raw == "" {
            return Err("'' is not a valid tag or value name".into());
        }

        let mut name = String::with_capacity(raw.len());

        let mut escaped = false;
        for chr in raw.chars() {
            if escaped {
                escaped = false;
                name.push(chr);
                continue;
            }

            match chr {
                '\\' => escaped = true,
                _ => name.push(chr),
            };
        }

        Ok(TagOrValueName { name })
    }
}

fn extract_names(parsed_names: &[TagOrValueName]) -> Vec<&str> {
    parsed_names.iter().map(|tovn| &tovn.name as &str).collect()
}

#[derive(Debug)]
struct TagAndValueNames {
    tag_name: String,
    value_name: Option<String>,
}

// Allow automatic parsing in StructOpt
impl str::FromStr for TagAndValueNames {
    type Err = Error;

    fn from_str(raw: &str) -> result::Result<Self, Self::Err> {
        let mut tag_name = String::with_capacity(raw.len());
        let mut value_name = String::with_capacity(raw.len());
        let mut name = &mut tag_name;

        // The borrow checker makes it hard to check whether "name" points to "tag_name" or
        // "value_name", so introduce an extra boolean
        let mut updating_tag = true;

        let mut escaped = false;
        for chr in raw.chars() {
            if escaped {
                escaped = false;
                (*name).push(chr);
                continue;
            }

            match chr {
                '\\' => escaped = true,
                '=' => {
                    if updating_tag {
                        name = &mut value_name;
                        updating_tag = false;
                    } else {
                        name.push(chr);
                    }
                }
                _ => name.push(chr),
            };
        }

        if tag_name.is_empty() {
            return Err("a tag name cannot be empty".into());
        }

        Ok(TagAndValueNames {
            tag_name,
            value_name: match &value_name as &str {
                "" => None,
                _ => Some(value_name),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn gen_examples() {
        // Single example without output
        assert_eq!(
            generate_examples_inner(false, &[("hello", None)]),
            "EXAMPLES:
    $ hello"
        );

        // Single example with multi-line output
        assert_eq!(
            generate_examples_inner(
                false,
                &[("command", Some("this is some\nmulti-line output"))]
            ),
            "EXAMPLES:
    $ command
    this is some
    multi-line output"
        );

        // Mixing examples with and without output
        assert_eq!(
            generate_examples_inner(
                false,
                &[
                    ("mkdir tmp-dir # inline comment", None),
                    ("cd tmp-dir", None),
                    ("tmsu init", Some("tmsu: /tmp/tmp-dir: creating database"))
                ]
            ),
            "EXAMPLES:
    $ mkdir tmp-dir # inline comment
    $ cd tmp-dir
    $ tmsu init
    tmsu: /tmp/tmp-dir: creating database"
        );

        // With colors
        assert_eq!(
            generate_examples_inner(true, &[("hello # comment", None)]),
            "\u{1b}[33mEXAMPLES:\u{1b}[0m
    \u{1b}[32m$\u{1b}[0m hello \u{1b}[36m# comment\u{1b}[0m"
        );
    }

    #[test]
    fn parse_tag_or_value_name() {
        // Helper function to remove some boilerplate
        fn assert_parse(raw: &str, expected_tag_name: &str) {
            let parsed = TagOrValueName::from_str(raw).unwrap();
            assert_eq!(parsed.name, expected_tag_name);
        }

        assert!(TagOrValueName::from_str("").is_err());

        assert_parse(r"abc", "abc");
        assert_parse(r"a or (b == c)", "a or (b == c)");
        assert_parse(r"\\\a\ or \(b =\= c)", r"\a or (b == c)");
        // TODO: similar to Go, but should perhaps be disallowed
        assert_parse(r"trailing\", "trailing");
    }

    #[test]
    fn parse_tag_and_value_names() {
        // Helper function to remove some boilerplate
        fn assert_parse(raw: &str, expected_tag_name: &str, expected_value_name: Option<&str>) {
            let parsed = TagAndValueNames::from_str(raw).unwrap();
            assert_eq!(parsed.tag_name, expected_tag_name);
            assert_eq!(parsed.value_name, expected_value_name.map(|s| s.to_owned()));
        }

        assert_parse(r"abc", "abc", None);
        assert_parse(r"abc=", "abc", None);
        assert_parse(r"abc\=", "abc=", None);
        assert_parse(r"a b=c d", "a b", Some("c d"));
        assert_parse(r"t=v1=v2", "t", Some("v1=v2"));
        assert_parse(r"\===", "=", Some("="));
        assert_parse(r"\t\\1\=t2=v1\=\v2", r"t\1=t2", Some("v1=v2"));
        // TODO: similar to Go, but should perhaps be disallowed
        assert_parse(r"tag=trailing\", "tag", Some("trailing"));

        // Tag names are mandatory
        assert!(TagAndValueNames::from_str("=abc").is_err());
    }

    #[test]
    fn test_rel_to() {
        fn assert_rel_path(base: &str, expected: &str) {
            let abs_base = AbsPath::from_unchecked(PathBuf::from(base));
            let abs_dir = AbsPath::from_unchecked(PathBuf::from("/some/path"));

            let actual = rel_to(&abs_dir, &abs_base).display().to_string();
            assert_eq!(
                actual, expected,
                "Expected '/some/path' relative to '{}' to be '{}' but was '{}'",
                base, expected, actual
            );
        }

        assert_rel_path("/", "./some/path");
        assert_rel_path("/other", "../some/path");
        assert_rel_path("/other/", "../some/path");
        assert_rel_path("/other/mother", "/some/path");
        assert_rel_path("/other/mother/", "/some/path");
        assert_rel_path("/some", "./path");
        assert_rel_path("/some/", "./path");
        assert_rel_path("/some/path", ".");
        assert_rel_path("/some/path/", ".");
        assert_rel_path("/some/path/subdir", "..");
        assert_rel_path("/some/path/subdir/", "..");
        assert_rel_path("/some/path/subdir/subsub", "/some/path");
        assert_rel_path("/some/cheese", "../path");
        assert_rel_path("/some/cheese/", "../path");
        assert_rel_path("/some/cheese/sandwich", "/some/path");
        assert_rel_path("/some/cheese/sandwich/", "/some/path");
    }
}
