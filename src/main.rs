extern crate getopts;
extern crate pest;
extern crate stache;

use std::env;
use std::fs::{self, File};
use std::io::{self, BufWriter, Error, ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::process::exit;

use getopts::Options;
use pest::{Parser, StringInput};
use stache::{Rdp, Statement, Template};
use stache::ruby;

enum Target {
    Ruby,
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut opts = Options::new();
    opts.optflag("h", "help", "Print this message");
    opts.reqopt("d", "", "Path to the template directory to compile", "PATH");
    opts.reqopt("o", "output", "Write output to FILE", "FILE");
    opts.reqopt("e", "emit", "Compile to a supported runtime: ruby", "LANG");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(e) => {
            usage(&opts);
            println!("{}", e);
            exit(1);
        }
    };

    if matches.opt_present("h") {
        usage(&opts);
        exit(0);
    }

    let base = match matches.opt_str("d") {
        Some(path) => PathBuf::from(path),
        None => {
            usage(&opts);
            exit(1);
        }
    };

    if !base.is_dir() {
        println!("Directory not found");
        exit(1);
    }

    let output = match matches.opt_str("o") {
        Some(path) => path,
        None => {
            usage(&opts);
            exit(1);
        }
    };

    let target = match matches.opt_str("e") {
        Some(lang) => {
            match lang.as_str() {
                "ruby" => Target::Ruby,
                _ => {
                    usage(&opts);
                    println!("Unsupported compilation target");
                    exit(1);
                }
            }
        }
        None => {
            usage(&opts);
            exit(1);
        }
    };

    let templates = match parse_dir(&base, &base) {
        Ok(templates) => templates,
        Err(e) => {
            println!("{}", e);
            exit(1);
        }
    };

    let source = match target {
        Target::Ruby => {
            match ruby::link(&templates) {
                Ok(node) => node,
                Err(e) => {
                    println!("{}", e);
                    exit(1);
                }
            }
        }
    };

    let done = File::create(output)
        .map(|file| BufWriter::new(file))
        .and_then(|mut buf| source.emit(&mut buf));

    match done {
        Ok(_) => (),
        Err(e) => {
            println!("{}", e);
            exit(1);
        }
    }
}

fn parse_dir(base: &PathBuf, dir: &PathBuf) -> io::Result<Vec<Template>> {
    let mut templates = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                templates.append(&mut parse_dir(base, &path)?);
            } else {
                let tree = parse(&path)?;
                let template = Template::new(&base, path, tree);
                templates.push(template);
            }
        }
    }
    Ok(templates)
}

fn parse(path: &Path) -> io::Result<Statement> {
    let mut file = File::open(path)?;
    let mut template = String::new();
    file.read_to_string(&mut template)?;

    let mut parser = Rdp::new(StringInput::new(&template));
    if parser.program() && parser.end() {
        match parser.tree() {
            Ok(tree) => Ok(tree),
            Err(e) => {
                let message = format!("Error parsing {:?}\n{}", path, e);
                Err(Error::new(ErrorKind::Other, message))
            }
        }
    } else {
        let (_, position) = parser.expected();
        let message = format!("Error parsing {:?} at position {}", path, position);
        Err(Error::new(ErrorKind::Other, message))
    }
}

fn usage(opts: &Options) {
    let brief = "Mustache template compiler\n\nUsage:\n    stache [options]";
    println!("{}", opts.usage(&brief));
}
