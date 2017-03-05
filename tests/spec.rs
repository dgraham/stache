extern crate stache;
extern crate tempdir;
extern crate yaml_rust;

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use stache::{Compile, Statement, Template};
use stache::ruby;

use tempdir::TempDir;
use yaml_rust::{Yaml, YamlLoader};

#[test]
fn ruby() {
    let build = TempDir::new("stache-build").unwrap();
    let source = build.path().join("stache.c");
    let script = "./tests/fixtures/test-ruby";

    let templates = templates();
    let program = ruby::link(&templates).unwrap();
    program.write(source).unwrap();

    let output = Command::new(script).arg(build.path()).output().unwrap();
    if !output.status.success() {
        let out = String::from_utf8(output.stdout).unwrap();
        let err = String::from_utf8(output.stderr).unwrap();
        panic!("{}{}", out, err);
    }
}

/// Parses templates provided by the Mustache specification suite.
fn templates() -> Vec<Template> {
    let base = PathBuf::from("ext/spec/specs");
    let files = vec!["comments", "interpolation", "inverted", "sections"];
    files.iter()
        .flat_map(|name| {
            let path = base.join(name).with_extension("yml");
            let spec = document(&path);
            let tests = spec["tests"].as_vec().unwrap();
            tests.iter()
                .enumerate()
                .map(|(index, test)| {
                    let template = test["template"].as_str().unwrap();
                    let tree = Statement::parse(template).unwrap();
                    let fake = path.with_file_name(format!("{}{}", name, index));
                    Template::new(&base, fake, tree)
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Parses the YAML document at the given path.
fn document(path: &Path) -> Yaml {
    let mut file = File::open(path).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();

    let mut docs = YamlLoader::load_from_str(&contents).unwrap();
    docs.pop().unwrap()
}
