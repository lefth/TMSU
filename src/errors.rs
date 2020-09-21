use std::convert;
use std::ffi::OsString;
use std::path::PathBuf;

use error_chain::error_chain;

error_chain! {
    errors {
        NoDatabaseFound(path: PathBuf) {
            description("No database found")
            display("No database found at '{}'", path.display())
        }
        DatabaseAccessError(path: PathBuf) {
            description("Cannot open database")
            display("Cannot open database at '{}'", path.display())
        }
        QueryParsingError(query: String) {
            description("Cannot parse query")
            display("Cannot parse query '{}'", &query)
        }
        OsStringConversion(os_string: OsString) {

        }
    }
    foreign_links {
        Chrono(chrono::ParseError);
        Io(std::io::Error);
        StripPrefix(std::path::StripPrefixError);
        Rusqlite(rusqlite::Error);
    }
}

impl convert::From<OsString> for Error {
    fn from(s: OsString) -> Self {
        ErrorKind::OsStringConversion(s).into()
    }
}
