#![recursion_limit = "70"]

#[macro_use]
extern crate pest;
extern crate regex;

use pest::prelude::*;
use std::fs::File;
use std::io::{self, BufWriter, Write};

pub use error::ParseError;
pub use name::Name;
pub use path::Path;
pub use template::Template;

pub mod ruby;
mod error;
mod name;
mod path;
mod template;

/// Defines the source code output behavior for compiler backends. The main
/// compiler driver treats the result of each backend identically.
pub trait Compile {
    /// Writes the final translated source code to an output buffer.
    fn emit(&self, buf: &mut Write) -> io::Result<()>;

    /// Saves the translated source code to a file.
    fn write<P>(&self, output: P) -> io::Result<()>
        where P: AsRef<std::path::Path>
    {
        File::create(output)
            .map(|file| BufWriter::new(file))
            .and_then(|mut buf| self.emit(&mut buf))
    }
}

#[derive(Debug, PartialEq)]
pub struct Block {
    statements: Vec<Statement>,
}

impl Block {
    fn new(statements: Vec<Statement>) -> Self {
        Block { statements: statements }
    }

    /// Adds the statement as the final element in the block, combining it with
    /// a previous content statement if possible.
    fn append(&mut self, statement: Statement) {
        let trailer = match self.statements.pop() {
            Some(mut last) => {
                if last.merge(&statement) {
                    last
                } else {
                    self.statements.push(last);
                    statement
                }
            }
            None => statement,
        };
        self.statements.push(trailer);
    }
}

#[derive(Debug, PartialEq)]
pub enum Statement {
    Program(Block),
    Section(Path, Block),
    Inverted(Path, Block),
    Variable(Path),
    Html(Path),
    Partial(String),
    Content(String),
    Comment(String),
}

impl Statement {
    /// Parses the Mustache text into a Statement AST.
    pub fn parse(template: &str) -> Result<Statement, ParseError> {
        let mut parser = Rdp::new(StringInput::new(template));
        if parser.program() && parser.end() {
            Ok(parser.tree())
        } else {
            let (_, position) = parser.expected();
            Err(ParseError::UnexpectedToken(position))
        }
    }

    /// Visits each node in the tree collecting the names of partials
    /// referenced by the template.
    pub fn partials<'a>(&'a self) -> Vec<&'a String> {
        match *self {
            Statement::Program(ref block) => {
                block.statements.iter().flat_map(|stmt| stmt.partials()).collect()
            }
            Statement::Section(_, ref block) => {
                block.statements.iter().flat_map(|stmt| stmt.partials()).collect()
            }
            Statement::Inverted(_, ref block) => {
                block.statements.iter().flat_map(|stmt| stmt.partials()).collect()
            }
            Statement::Partial(ref name) => vec![name],
            _ => Vec::new(),
        }
    }

    /// Combines adjacent content statements into a single statement.
    ///
    /// Returns true if the statements were merged.
    fn merge(&mut self, statement: &Statement) -> bool {
        match *self {
            Statement::Content(ref mut left) => {
                match *statement {
                    Statement::Content(ref right) => {
                        left.push_str(right);
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }
}

impl_rdp! {
    grammar! {
        program     = { block ~ eoi }
        block       = @{ statement* }
        statement   = { comment | content | variable | html | section | inverted | partial }
        content     = { (!open ~ any)+ }
        variable    = !@{ open ~ path ~ close }
        html        = !@{ (["{{{"] ~ path ~ ["}}}"]) | (["{{&"] ~ path ~ close) }
        section     = @{ sopen ~ block ~ sclose }
        sopen       = !@{ ["{{#"] ~ [push(path)] ~ close }
        sclose      = !@{ ["{{/"] ~ [pop()] ~ close }
        inverted    = @{ invopen ~ block ~ sclose }
        invopen     = !@{ ["{{^"] ~ [push(path)] ~ close }
        comment     = { ["{{!"] ~ ctext ~ close }
        ctext       = { (!close ~ any)* }
        partial     = !@{ ["{{>"] ~ partial_id ~ close }
        partial_id  = { (['a'..'z'] | ['A'..'Z'] | ['0'..'9'] | ["-"] | ["_"] | ["/"])+ }
        open        = _{ ["{{"] }
        close       = _{ ["}}"] }
        path        = @{ dot | (identifier ~ (["."] ~ identifier)*) }
        dot         = { ["."] }
        identifier  = { (['a'..'z'] | ['A'..'Z'] | ['0'..'9'] | ["-"] | ["_"] | ["?"] | ["!"])+ }
        whitespace  = _{ [" "] | ["\t"] | ["\r"] | ["\n"] }
    }

    process! {
        tree(&self) -> Statement {
            (_: program, block: _block()) => {
                Statement::Program(block)
            }
        }

        _block(&self) -> Block {
            (_: block, list: _statements()) => {
                Block::new(list)
            }
        }

        _statements(&self) -> Vec<Statement> {
            (_: statement, head: _statement(), mut tail: _statements()) => {
                tail.insert(0, head);
                tail
            },
            () => {
                Vec::new()
            }
        }

        _statement(&self) -> Statement {
            (_: comment, &text: ctext) => {
                Statement::Comment(String::from(text))
            },
            (&text: content) => {
                Statement::Content(String::from(text))
            },
            (_: variable, path: _path()) => {
                Statement::Variable(path)
            },
            (_: html, path: _path()) => {
                Statement::Html(path)
            },
            (_: partial, &name: partial_id) => {
                Statement::Partial(String::from(name))
            },
            (_: section, _: sopen, path: _path(), block: _block(), _: sclose) => {
                Statement::Section(path, block)
            },
            (_: inverted, _: invopen, path: _path(), block: _block(), _: sclose) => {
                Statement::Inverted(path, block)
            }
        }

        _path(&self) -> Path {
            (_: path, _: dot) => {
                Path::new(vec![String::from(".")])
            },
            (_: path, list: _identifier()) => {
                Path::new(list)
            }
        }

        _identifier(&self) -> Vec<String> {
            (&head: identifier, mut tail: _identifier()) => {
                tail.insert(0, String::from(head));
                tail
            },
            () => {
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append() {
        let mut block = Block::new(vec![Statement::Comment("a".into())]);
        block.append(Statement::Content("b".into()));
        let expected = Block::new(vec![Statement::Comment("a".into()),
                                       Statement::Content("b".into())]);
        assert_eq!(expected, block);
    }

    #[test]
    fn append_and_merge() {
        let mut block = Block::new(vec![Statement::Content("a".into())]);
        block.append(Statement::Content("b".into()));
        assert_eq!(Block::new(vec![Statement::Content("ab".into())]), block);
    }

    #[test]
    fn merge() {
        let mut a = Statement::Content("a".into());
        let b = Statement::Content("b".into());
        assert!(a.merge(&b));
        assert_eq!(Statement::Content("ab".into()), a);
    }

    #[test]
    fn identifier() {
        let mut parser = Rdp::new(StringInput::new("abc?"));
        assert!(parser.identifier());
        assert!(parser.end());

        let expected = vec![Token::new(Rule::identifier, 0, 4)];
        assert_eq!(&expected, parser.queue());
    }

    #[test]
    fn path() {
        let mut parser = Rdp::new(StringInput::new("a.b.c!"));
        assert!(parser.path());
        assert!(parser.end());

        let expected = vec![Token::new(Rule::path, 0, 6),
                            Token::new(Rule::identifier, 0, 1),
                            Token::new(Rule::identifier, 2, 3),
                            Token::new(Rule::identifier, 4, 6)];
        assert_eq!(&expected, parser.queue());
    }

    #[test]
    fn partial_id() {
        let mut parser = Rdp::new(StringInput::new("a/b/c"));
        assert!(parser.partial_id());
        assert!(parser.end());

        let expected = vec![Token::new(Rule::partial_id, 0, 5)];
        assert_eq!(&expected, parser.queue());
    }

    #[test]
    fn partial() {
        let mut parser = Rdp::new(StringInput::new("{{> a/b}}"));
        assert!(parser.partial());
        assert!(parser.end());

        let expected = vec![Token::new(Rule::partial, 0, 9), Token::new(Rule::partial_id, 4, 7)];
        assert_eq!(&expected, parser.queue());
    }

    #[test]
    fn comment() {
        let mut parser = Rdp::new(StringInput::new("{{! a b c}}"));
        assert!(parser.comment());
        assert!(parser.end());

        let expected = vec![Token::new(Rule::comment, 0, 11), Token::new(Rule::ctext, 3, 9)];
        assert_eq!(&expected, parser.queue());
    }

    #[test]
    fn inverted() {
        let mut parser = Rdp::new(StringInput::new("{{^ a}}{{/ a}}"));
        assert!(parser.inverted());
        assert!(parser.end());

        let expected = vec![Token::new(Rule::inverted, 0, 14),
                            Token::new(Rule::invopen, 0, 7),
                            Token::new(Rule::path, 4, 5),
                            Token::new(Rule::identifier, 4, 5),
                            Token::new(Rule::block, 7, 7),
                            Token::new(Rule::sclose, 7, 14)];
        assert_eq!(&expected, parser.queue());
    }

    #[test]
    fn section() {
        let mut parser = Rdp::new(StringInput::new("{{# a}}{{/ a}}"));
        assert!(parser.section());
        assert!(parser.end());

        let expected = vec![Token::new(Rule::section, 0, 14),
                            Token::new(Rule::sopen, 0, 7),
                            Token::new(Rule::path, 4, 5),
                            Token::new(Rule::identifier, 4, 5),
                            Token::new(Rule::block, 7, 7),
                            Token::new(Rule::sclose, 7, 14)];
        assert_eq!(&expected, parser.queue());
    }

    #[test]
    #[should_panic]
    fn invalid_section() {
        let mut parser = Rdp::new(StringInput::new("{{#one}}test{{/two}}"));
        assert!(parser.section());
    }

    #[test]
    fn variable() {
        let mut parser = Rdp::new(StringInput::new("{{ a }}"));
        assert!(parser.variable());
        assert!(parser.end());

        let expected = vec![Token::new(Rule::variable, 0, 7),
                            Token::new(Rule::path, 3, 4),
                            Token::new(Rule::identifier, 3, 4)];
        assert_eq!(&expected, parser.queue());
    }

    #[test]
    fn dot() {
        let mut parser = Rdp::new(StringInput::new("{{ . }}"));
        assert!(parser.variable());
        assert!(parser.end());

        let expected = vec![Token::new(Rule::variable, 0, 7),
                            Token::new(Rule::path, 3, 4),
                            Token::new(Rule::dot, 3, 4)];
        assert_eq!(&expected, parser.queue());
    }

    #[test]
    fn html() {
        let mut parser = Rdp::new(StringInput::new("{{{ a }}}"));
        assert!(parser.html());
        assert!(parser.end());

        let expected = vec![Token::new(Rule::html, 0, 9),
                            Token::new(Rule::path, 4, 5),
                            Token::new(Rule::identifier, 4, 5)];
        assert_eq!(&expected, parser.queue());
    }

    #[test]
    fn ampersand() {
        let mut parser = Rdp::new(StringInput::new("{{& a }}"));
        assert!(parser.html());
        assert!(parser.end());

        let expected = vec![Token::new(Rule::html, 0, 8),
                            Token::new(Rule::path, 4, 5),
                            Token::new(Rule::identifier, 4, 5)];
        assert_eq!(&expected, parser.queue());
    }

    #[test]
    fn whitespace() {
        let mut parser = Rdp::new(StringInput::new("{{ a }} b"));
        assert!(parser.program());
        assert!(parser.end());

        let expected = vec![Token::new(Rule::program, 0, 9),
                            Token::new(Rule::block, 0, 9),
                            Token::new(Rule::statement, 0, 7),
                            Token::new(Rule::variable, 0, 7),
                            Token::new(Rule::path, 3, 4),
                            Token::new(Rule::identifier, 3, 4),
                            Token::new(Rule::statement, 7, 9),
                            Token::new(Rule::content, 7, 9)];
        assert_eq!(&expected, parser.queue());
    }

    #[test]
    fn program() {
        let mut parser = Rdp::new(StringInput::new("
            {{> includes/header }}
            <ul>
                {{# robots}}
                    <li>{{ name.first }}</li>
                {{/ robots}}
                {{^ robots}}
                    {{! else clause }}
                    No robots
                {{/ robots}}
            </ul>
            {{> includes/footer }}
            {{{ unescaped.html }}}
        "));
        assert!(parser.program());
        assert!(parser.end());

        let expected = vec![Token::new(Rule::program, 0, 380),
                            Token::new(Rule::block, 0, 380),
                            Token::new(Rule::statement, 0, 13),
                            Token::new(Rule::content, 0, 13),
                            Token::new(Rule::statement, 13, 35),
                            Token::new(Rule::partial, 13, 35),
                            Token::new(Rule::partial_id, 17, 32),
                            Token::new(Rule::statement, 35, 69),
                            Token::new(Rule::content, 35, 69),
                            Token::new(Rule::statement, 69, 156),
                            Token::new(Rule::section, 69, 156),
                            Token::new(Rule::sopen, 69, 81),
                            Token::new(Rule::path, 73, 79),
                            Token::new(Rule::identifier, 73, 79),
                            Token::new(Rule::block, 81, 144),
                            Token::new(Rule::statement, 81, 106),
                            Token::new(Rule::content, 81, 106),
                            Token::new(Rule::statement, 106, 122),
                            Token::new(Rule::variable, 106, 122),
                            Token::new(Rule::path, 109, 119),
                            Token::new(Rule::identifier, 109, 113),
                            Token::new(Rule::identifier, 114, 119),
                            Token::new(Rule::statement, 122, 144),
                            Token::new(Rule::content, 122, 144),
                            Token::new(Rule::sclose, 144, 156),
                            Token::new(Rule::statement, 156, 173),
                            Token::new(Rule::content, 156, 173),
                            Token::new(Rule::statement, 173, 283),
                            Token::new(Rule::inverted, 173, 283),
                            Token::new(Rule::invopen, 173, 185),
                            Token::new(Rule::path, 177, 183),
                            Token::new(Rule::identifier, 177, 183),
                            Token::new(Rule::block, 185, 271),
                            Token::new(Rule::statement, 185, 206),
                            Token::new(Rule::content, 185, 206),
                            Token::new(Rule::statement, 206, 224),
                            Token::new(Rule::comment, 206, 224),
                            Token::new(Rule::ctext, 209, 222),
                            Token::new(Rule::statement, 224, 271),
                            Token::new(Rule::content, 224, 271),
                            Token::new(Rule::sclose, 271, 283),
                            Token::new(Rule::statement, 283, 314),
                            Token::new(Rule::content, 283, 314),
                            Token::new(Rule::statement, 314, 336),
                            Token::new(Rule::partial, 314, 336),
                            Token::new(Rule::partial_id, 318, 333),
                            Token::new(Rule::statement, 336, 349),
                            Token::new(Rule::content, 336, 349),
                            Token::new(Rule::statement, 349, 371),
                            Token::new(Rule::html, 349, 371),
                            Token::new(Rule::path, 353, 367),
                            Token::new(Rule::identifier, 353, 362),
                            Token::new(Rule::identifier, 363, 367),
                            Token::new(Rule::statement, 371, 380),
                            Token::new(Rule::content, 371, 380)];
        assert_eq!(&expected, parser.queue());
    }

    #[test]
    fn tree() {
        let mut parser = Rdp::new(StringInput::new("
            {{> includes/header }}
            <ul>
                {{# robots}}
                    <li>{{ name.first }}</li>
                {{/ robots}}
                {{^ robots}}
                    {{! else clause }}
                    No robots
                {{/ robots}}
            </ul>
            {{> includes/footer }}
            {{{ unescaped.html }}}
        "));

        assert!(parser.program());
        assert!(parser.end());

        let list = vec![Statement::Content(String::from("\n                    <li>")),
                        Statement::Variable(Path::new(vec![String::from("name"),
                                                           String::from("first")])),
                        Statement::Content(String::from("</li>\n                "))];

        let section = Statement::Section(Path::new(vec![String::from("robots")]), Block::new(list));


        let invblock = vec![Statement::Content(String::from("\n                    ")),
                            Statement::Comment(String::from(" else clause ")),
                            Statement::Content(String::from("\n                    No robots\n                \
                                                             "))];
        let inverted = Statement::Inverted(Path::new(vec![String::from("robots")]),
                                           Block::new(invblock));

        let program =
            vec![Statement::Content(String::from("\n            ")),
                 Statement::Partial(String::from("includes/header")),
                 Statement::Content(String::from("\n            <ul>\n                ")),
                 section,
                 Statement::Content(String::from("\n                ")),
                 inverted,
                 Statement::Content(String::from("\n            </ul>\n            ")),
                 Statement::Partial(String::from("includes/footer")),
                 Statement::Content(String::from("\n            ")),
                 Statement::Html(Path::new(vec![String::from("unescaped"), String::from("html")])),
                 Statement::Content(String::from("\n        "))];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }
}
