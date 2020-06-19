use std::fmt;
use std::ops;

use lazy_static::lazy_static;
use regex::Regex;

use crate::errors::*;

// Initialize the regular expression only once, and on demand
lazy_static! {
    // Valid chars are the union of the following Unicode classes:
    //  * Letter
    //  * Numeric
    //  * Punctuation
    //  * Symbol
    //  * Space
    // This expression is negated to match invalid characters
    static ref INVALID_CHARS: Regex = Regex::new(r"[^\pL\pN\pP\pS\s]").unwrap();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TagId(pub u32);

impl fmt::Display for TagId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A value ID, which cannot be 0. See `OptionalValueId` for more details.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(u32);

impl ValueId {
    /// Create a ValueId, but panics if its ID is 0.
    /// If you have an ID which is possibly 0, you probably want
    /// to use the OptionalValueId type instead
    pub fn from_unchecked(id: u32) -> Self {
        assert!(id != 0, "A ValueId cannot be 0");
        Self(id)
    }

    pub fn as_u32(&self) -> &u32 {
        &self.0
    }
}

impl fmt::Display for ValueId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// This wrapper around Option<ValueId> is there to improve type safety.
///
/// In the "filetag" table, a missing value is represented with an ID of 0 instead of a NULL (not
/// sure why, maybe to make joins easier?). If we only used a ValueId type similar to TagId, there
/// would be way to know that the value ID is missing other than checking it in the "right" places.
///
/// Using an Option<ValueId> is better than having to remember to check for the magic 0 value
/// everywhere. Unfortunately, rusqlite has a default implementation of the ToSql trait for
/// Option<T>, which means we might be mapping it to NULL instead of 0 if we forgot to convert it
/// explicitly. So we would end up with a similar issue, moved to the DB layer.
///
/// Using a wrapper around Option<ValueId> avoids this problem, since ToSql is not implemented for
/// this new type. We can then implement the trait the way we want (this is done in storage.rs).
///
/// To enforce this design, a ValueId cannot hold an ID of 0. This means that a function should
/// accept a ValueId (rather than an OptionalValueId) if and only if it handles non-zero
/// (semantically non-null) IDs exclusively.
///
/// Ideally, the DB would store NULL instead of 0 and we would get rid of this workaround, but that
/// would be a lot more work, if at all feasible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OptionalValueId(Option<ValueId>);

impl OptionalValueId {
    pub fn as_u32(&self) -> &u32 {
        match self.0.as_ref() {
            None => &0,
            Some(ValueId(id)) => id,
        }
    }

    pub fn from_id(id: u32) -> Self {
        // A value ID of 0 in the DB actually means no value...
        let opt = match id {
            0 => None,
            i => Some(ValueId::from_unchecked(i)),
        };
        Self { 0: opt }
    }
}

impl ops::Deref for OptionalValueId {
    type Target = Option<ValueId>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(pub u32);

impl fmt::Display for FileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

pub struct Tag {
    pub id: TagId,
    pub name: String,
}

pub struct Value {
    pub id: ValueId,
    pub name: String,
}

pub struct FileTag {
    pub file_id: FileId,
    pub tag_id: TagId,
    pub value_id: OptionalValueId,
    pub explicit: bool,
    pub implicit: bool,
}

pub struct TagFileCount {
    pub id: TagId,
    pub name: String,
    pub file_count: u32,
}

pub fn validate_tag_name(name: &str) -> Result<()> {
    validate_name_helper("tag names", name)
}

pub fn validate_value_name(name: &str) -> Result<()> {
    validate_name_helper("tag value", name)
}

/// Validate that the given name is a valid `what`.
/// This helper function is only there to avoid duplication, since tag names and tag values
/// currently have the same rules.
fn validate_name_helper(what: &str, name: &str) -> Result<()> {
    let error_message = match name {
        "" => Some("cannot be empty"),
        // Cannot be used in the VFS
        "." | ".." => Some("cannot be '.' or '..'"),
        // Used in the query language
        "and" | "AND" | "or" | "OR" | "not" | "NOT" => {
            Some("cannot be a logical operator: 'and', 'or' or 'not'")
        }
        // Used in the query language
        "eq" | "EQ" | "ne" | "NE" | "lt" | "LT" | "gt" | "GT" | "le" | "LE" | "ge" | "GE" => {
            Some("cannot be a comparison operator: 'eq', 'ne', 'gt', 'lt', 'ge' or 'le'")
        }
        _ => None,
    };
    if let Some(message) = error_message {
        return Err(format!("{} {}", what, message).into());
    }

    // Check Unicode characters
    if let Some(mat) = INVALID_CHARS.find(name) {
        // Unwrapping is safe because the regular expression always matches at least one character
        let bad_char = mat.as_str().chars().next().unwrap();
        let message = format!("{} cannot contain U+{:04X}", what, bad_char as u32);
        return Err(message.into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_tag_name() {
        // "Normal" cases
        assert!(validate_tag_name("abc").is_ok());
        assert!(validate_tag_name("name='(1 or 2) lt 3'").is_ok());

        // Empty string
        assert!(validate_tag_name("").is_err());

        // Dots
        assert!(validate_tag_name(".").is_err());
        assert!(validate_tag_name("..").is_err());
        assert!(validate_tag_name("...").is_ok());
        assert!(validate_tag_name(".. ..").is_ok());

        // Logical and comparison operators: case matters
        assert!(validate_tag_name("and").is_err());
        assert!(validate_tag_name("AND").is_err());
        assert!(validate_tag_name("AnD").is_ok());
        assert!(validate_tag_name(" and ").is_ok());

        // Special characters and Unicode
        assert!(validate_tag_name(" \t\n\r ").is_ok());
        assert!(validate_tag_name("éüßżć").is_ok());
        assert!(validate_tag_name("今日は!").is_ok());
        assert!(validate_tag_name("control har").is_err());
    }

    #[test]
    fn test_validate_value_name() {
        // "Normal" cases
        assert!(validate_value_name("abc").is_ok());
        assert!(validate_value_name("name='(1 or 2) lt 3'").is_ok());

        // Empty string
        assert!(validate_value_name("").is_err());

        // Dots
        assert!(validate_value_name(".").is_err());
        assert!(validate_value_name("..").is_err());
        assert!(validate_value_name("...").is_ok());
        assert!(validate_value_name(".. ..").is_ok());

        // Logical and comparison operators: case matters
        assert!(validate_value_name("and").is_err());
        assert!(validate_value_name("AND").is_err());
        assert!(validate_value_name("AnD").is_ok());
        assert!(validate_value_name(" and ").is_ok());

        // Special characters and Unicode
        assert!(validate_value_name(" \t\n\r ").is_ok());
        assert!(validate_value_name("éüßżć").is_ok());
        assert!(validate_value_name("今日は!").is_ok());
        assert!(validate_value_name("control har").is_err());
    }
}
