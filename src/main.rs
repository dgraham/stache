extern crate pest;
extern crate stache;

use std::io::{self, Read};
use std::process::exit;

use pest::prelude::*;
use stache::Rdp;

fn main() {
    let mut template = String::new();
    io::stdin().read_to_string(&mut template).unwrap();

    let mut parser = Rdp::new(StringInput::new(&template));
    if parser.program() && parser.end() {
        for x in parser.queue() {
            println!("{:?}", x);
        }
    } else {
        let (expected, position) = parser.expected();
        println!("Expected at position {}: ", position);
        for rule in expected {
            println!("{:?}", rule);
        }
        exit(1);
    }
}
