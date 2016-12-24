use std::fmt;

#[derive(Debug, PartialEq)]
pub struct Path {
    pub keys: Vec<String>,
}

impl Path {
    pub fn new(keys: Vec<String>) -> Self {
        Path { keys: keys }
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.keys.join("."))
    }
}

#[cfg(test)]
mod tests {
    use super::Path;

    #[test]
    fn to_string() {
        let path = Path::new(vec![String::from("one"), String::from("two")]);
        assert_eq!("one.two", path.to_string());
    }
}
