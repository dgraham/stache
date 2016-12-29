use std::error::Error;
use std::fmt;
use std::path::PathBuf;
use super::Path;

#[derive(Debug)]
pub enum ParseError {
    InvalidSection(Path, Path),
    UnexpectedToken(usize),
    UnknownPartial(String, PathBuf),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ParseError::InvalidSection(ref open, ref close) => {
                write!(f, "Section open and close must match: {}, {}", open, close)
            }
            ParseError::UnexpectedToken(position) => {
                write!(f, "Unexpected token at position {}", position)
            }
            ParseError::UnknownPartial(ref name, ref path) => {
                write!(f, "Undefined partial `{}` called in {:?}", name, path)
            }
        }
    }
}

impl Error for ParseError {
    fn description(&self) -> &str {
        match *self {
            ParseError::InvalidSection(..) => "Section open and close must match",
            ParseError::UnexpectedToken(_) => "Unexpected token",
            ParseError::UnknownPartial(..) => "Undefined partial called",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            _ => None,
        }
    }
}
