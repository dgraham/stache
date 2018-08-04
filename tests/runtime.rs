extern crate stache;
extern crate tempdir;

use std::io::Error;
use std::process::Command;
use tempdir::TempDir;

use stache::ruby;
use stache::{Compile, Template};

#[test]
fn ruby() {
    let build = build("tests/fixtures/templates").unwrap();
    let script = "./tests/fixtures/test-runtime.rb";

    let output = Command::new(script).arg(build.path()).output().unwrap();
    if !output.status.success() {
        let out = String::from_utf8(output.stdout).unwrap();
        let err = String::from_utf8(output.stderr).unwrap();
        panic!("{}{}", out, err);
    }
}

#[ignore]
#[test]
fn bench_ruby() {
    let build = build("tests/fixtures/benches").unwrap();
    let script = "./tests/fixtures/bench-ruby";

    let output = Command::new(script).arg(build.path()).output().unwrap();
    let out = String::from_utf8(output.stdout).unwrap();
    let err = String::from_utf8(output.stderr).unwrap();
    println!("{}{}", out, err);
}

/// Compile the template directory into a Ruby extension source file.
///
/// Returns the source file's temporary directory to be passed to the Ruby
/// test scripts for final compilation.
fn build(path: &str) -> Result<TempDir, Error> {
    let build = TempDir::new("stache-build")?;
    let source = build.path().join("stache.c");

    let templates = Template::parse(path)?;
    let program = ruby::link(&templates).unwrap();
    program.write(source)?;

    Ok(build)
}
