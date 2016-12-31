extern crate stache;
extern crate tempdir;

use std::process::Command;
use tempdir::TempDir;

use stache::{Compile, Template};
use stache::ruby;

#[test]
fn ruby() {
    let build = TempDir::new("stache-build").unwrap();
    let source = build.path().join("stache.c");
    let script = "./tests/fixtures/test-runtime.rb";

    let templates = Template::parse("tests/fixtures/templates").unwrap();
    let program = ruby::link(&templates).unwrap();
    program.write(source).unwrap();

    let output = Command::new(script).arg(build.path()).output().unwrap();
    if !output.status.success() {
        let errors = String::from_utf8(output.stdout).unwrap();
        panic!(errors);
    }
}
