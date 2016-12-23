extern crate regex;

use regex::Regex;
use std::fmt;

/// An identifier name generator.
pub struct Name {
    base: String,
    next: usize,
}

impl Name {
    /// Initialize a name generator with a base name. A unique identifier may
    /// then be generated with the `next` and `to_string` functions.
    pub fn new(base: &str) -> Self {
        Name {
            base: String::from(base),
            next: 0,
        }
    }

    /// Advances the generator to the next unique identifier. When passing
    /// a `Name` through recursive function calls, this can be called before
    /// the next recursion to increment the depth of the generated
    /// identifiers.
    pub fn next(&mut self) -> &mut Self {
        self.next = self.next + 1;
        self
    }
}

impl Name {
    /// Creates a valid identifier from the template's short name to be used
    /// in function or variable names generated from this template file:
    /// `include/header -> include_header`.
    pub fn id(&self) -> String {
        let re = Regex::new(r"[^\w]").unwrap();
        re.replace_all(&self.base, "_")
    }
}

impl fmt::Display for Name {
    /// Creates a unique identifier to be used as a variable or function name.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}", self.id(), self.next)
    }
}

#[cfg(test)]
mod tests {
    use super::Name;

    #[test]
    fn id() {
        let name = Name::new("include/header");
        assert_eq!("include_header", name.id());
    }

    #[test]
    fn next() {
        let mut name = Name::new("include/header");
        assert_eq!("include_header0", name.to_string());

        name.next();
        assert_eq!("include_header1", name.to_string());

        name.next();
        assert_eq!("include_header2", name.to_string());
    }
}
