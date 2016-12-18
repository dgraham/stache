use std::error::Error;
use std::fmt;
use path::Path;

#[derive(Debug)]
pub enum ParseError {
    InvalidSection(Path, Path),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ParseError::InvalidSection(ref open, ref close) => {
                write!(f, "Section open and close must match: {}, {}", open, close)
            }
        }
    }
}

impl Error for ParseError {
    fn description(&self) -> &str {
        match *self {
            ParseError::InvalidSection(..) => "Section open and close must match",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            _ => None,
        }
    }
}
