use std::path::{Path, PathBuf};
use super::{Name, Statement};

// A binding of template source file information and the parsed AST.
#[derive(Debug)]
pub struct Template {
    pub tree: Statement,
    pub path: PathBuf,
    pub name: String,
    id: String,
}

impl Template {
    /// Creates a template from file name and root AST node.
    ///
    /// The file name is used as an identifier in compiled function names
    /// to ensure uniqueness when linked with other templates. It provides
    /// a stable name to be referenced as a partial in other templates.
    pub fn new(base: &PathBuf, path: PathBuf, tree: Statement) -> Self {
        let name = name(base, &path);
        let id = Name::new(&name).id();

        Template {
            tree: tree,
            path: path,
            name: name,
            id: id,
        }
    }

    pub fn name(&self) -> Name {
        Name::new(&self.name)
    }
}

/// Creates a shortened path name for a template file name. The base directory
/// being compiled and the file extension is stripped off to create the short
/// name: `app/templates/include/header.mustache -> include/header`.
fn name(base: &Path, path: &Path) -> String {
    let base = path.strip_prefix(base).unwrap();
    let stem = base.file_stem().unwrap();
    let name = base.with_file_name(stem);
    String::from(name.to_str().unwrap())
}

#[cfg(test)]
mod tests {
    use super::Template;
    use super::super::Statement;
    use std::path::PathBuf;

    #[test]
    fn name() {
        let base = PathBuf::from("app/templates");
        let path = PathBuf::from("app/templates/include/header.mustache");
        let tree = Statement::Content(String::from("test"));

        let template = Template::new(&base, path, tree);
        assert_eq!("include/header", template.name);
        assert_eq!("include_header", template.id);
    }
}
