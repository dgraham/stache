use std::error::Error;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ParseError {
    UnexpectedToken(usize),
    UnknownPartial(String, PathBuf),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
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
            ParseError::UnexpectedToken(_) => "Unexpected token",
            ParseError::UnknownPartial(..) => "Undefined partial called",
        }
    }

    fn cause(&self) -> Option<&dyn Error> {
        match *self {
            _ => None,
        }
    }
}
