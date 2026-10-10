use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PodError {
    /// The archive ends before something it says it contains.
    Truncated {
        what: String,
        offset: usize,
        needed: usize,
        available: usize,
    },
    /// A file that another file refers to isn't in the archive.
    MissingFile(String),
    /// A text file doesn't have something it must have.
    MissingField { field: String },
    /// A value couldn't be understood. `line` counts from 1.
    BadValue {
        field: String,
        line: usize,
        text: String,
    },
    /// A kind of file this crate doesn't read.
    Unsupported(String),
    /// A binary file has a size that doesn't fit its format.
    BadSize { expected: String, actual: usize },
    /// Any of the above, with the archive path of the file it happened in.
    InFile { file: String, error: Box<PodError> },
}

impl PodError {
    pub(super) fn in_file(self, file: &str) -> Self {
        Self::InFile {
            file: file.to_string(),
            error: Box::new(self),
        }
    }
}

impl fmt::Display for PodError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated {
                what,
                offset,
                needed,
                available,
            } => write!(
                f,
                "{what} at offset {offset} needs {needed} bytes but only {available} remain"
            ),
            Self::MissingFile(file) => write!(f, "{file} is not in the archive"),
            Self::MissingField { field } => write!(f, "no \"{field}\" entry"),
            // Binary files have no lines to number.
            Self::BadValue {
                field,
                line: 0,
                text,
            } => {
                write!(f, "cannot read \"{text}\" as {field}")
            }
            Self::BadValue { field, line, text } => {
                write!(f, "line {line}: cannot read \"{text}\" as {field}")
            }
            Self::Unsupported(what) => write!(f, "is {what}"),
            Self::BadSize { expected, actual } => {
                write!(f, "is {actual} bytes, expected {expected}")
            }
            Self::InFile { file, error } => write!(f, "{file}: {error}"),
        }
    }
}

impl std::error::Error for PodError {}
