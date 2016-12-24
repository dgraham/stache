#[macro_use]
extern crate pest;
extern crate regex;

use pest::prelude::*;

pub use error::ParseError;
pub use name::Name;
pub use path::Path;
pub use template::Template;

pub mod ruby;
mod error;
mod name;
mod path;
mod template;

#[derive(Debug, PartialEq)]
pub struct Block {
    statements: Vec<Statement>,
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
}

impl Statement {
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
            Statement::Partial(ref name) => {
                vec![name]
            }
            _ => Vec::new(),
        }
    }
}

impl_rdp! {
    grammar! {
        program     = { block ~ eoi }
        block       = { statement* }
        statement   = { content | variable | html | section | inverted | partial }
        content     = @{ (!open ~ any)+ }
        variable    = { open ~ path ~ close }
        html        = { ["{{{"] ~ path ~ ["}}}"] }
        section     = { ["{{#"] ~ path ~ close ~ block ~ ["{{/"] ~ path ~ close }
        inverted    = { ["{{^"] ~ path ~ close ~ block ~ ["{{/"] ~ path ~ close }
        comment     = _{ ["{{!"] ~ (!close ~ any)* ~ close }
        partial     = { ["{{>"] ~ partial_id ~ close }
        partial_id  = { (['a'..'z'] | ['A'..'Z'] | ['0'..'9'] | ["-"] | ["_"] | ["/"])+ }
        open        = _{ ["{{"] }
        close       = _{ ["}}"] }
        path        = @{ identifier ~ (["."] ~ identifier)* }
        identifier  = { (['a'..'z'] | ['A'..'Z'] | ['0'..'9'] | ["-"] | ["_"])+ }
        whitespace  = _{ [" "] | ["\t"] | ["\r"] | ["\n"]}
    }

    process! {
        tree(&self) -> Result<Statement, ParseError> {
            (_: program, block: _block()) => {
                Ok(Statement::Program(block?))
            }
        }

        _block(&self) -> Result<Block, ParseError> {
            (_: block, list: _statements()) => {
                Ok(Block { statements: list? })
            }
        }

        _statements(&self) -> Result<Vec<Statement>, ParseError> {
            (_: statement, head: _statement(), tail: _statements()) => {
                match tail {
                    Ok(mut tail) => {
                        tail.insert(0, head?);
                        Ok(tail)
                    }
                    Err(e) => Err(e),
                }
            },
            () => {
                Ok(Vec::new())
            }
        }

        _statement(&self) -> Result<Statement, ParseError> {
            (&text: content) => {
                Ok(Statement::Content(String::from(text)))
            },
            (_: variable, path: _path()) => {
                Ok(Statement::Variable(path))
            },
            (_: html, path: _path()) => {
                Ok(Statement::Html(path))
            },
            (_: partial, &name: partial_id) => {
                Ok(Statement::Partial(String::from(name)))
            },
            (_: section, open: _path(), block: _block(), close: _path()) => {
                if open != close {
                    return Err(ParseError::InvalidSection(open, close));
                }
                Ok(Statement::Section(open, block?))
            },
            (_: inverted, open: _path(), block: _block(), close: _path()) => {
                if open != close {
                    return Err(ParseError::InvalidSection(open, close));
                }
                Ok(Statement::Inverted(open, block?))
            }
        }

        _path(&self) -> Path {
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
    use pest::prelude::*;
    use super::{Block, Path, ParseError, Rdp, Rule, Statement};

    #[test]
    fn identifier() {
        let mut parser = Rdp::new(StringInput::new("abc"));
        assert!(parser.identifier());
        assert!(parser.end());

        let expected = vec![Token::new(Rule::identifier, 0, 3)];
        assert_eq!(&expected, parser.queue());
    }

    #[test]
    fn path() {
        let mut parser = Rdp::new(StringInput::new("a.b.c"));
        assert!(parser.path());
        assert!(parser.end());

        let expected = vec![Token::new(Rule::path, 0, 5),
                            Token::new(Rule::identifier, 0, 1),
                            Token::new(Rule::identifier, 2, 3),
                            Token::new(Rule::identifier, 4, 5)];
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
        assert!(parser.queue().is_empty());
    }

    #[test]
    fn inverted() {
        let mut parser = Rdp::new(StringInput::new("{{^ a}}{{/ a}}"));
        assert!(parser.inverted());
        assert!(parser.end());

        let expected = vec![Token::new(Rule::inverted, 0, 14),
                            Token::new(Rule::path, 4, 5),
                            Token::new(Rule::identifier, 4, 5),
                            Token::new(Rule::path, 11, 12),
                            Token::new(Rule::identifier, 11, 12)];
        assert_eq!(&expected, parser.queue());
    }

    #[test]
    fn section() {
        let mut parser = Rdp::new(StringInput::new("{{# a}}{{/ a}}"));
        assert!(parser.section());
        assert!(parser.end());

        let expected = vec![Token::new(Rule::section, 0, 14),
                            Token::new(Rule::path, 4, 5),
                            Token::new(Rule::identifier, 4, 5),
                            Token::new(Rule::path, 11, 12),
                            Token::new(Rule::identifier, 11, 12)];
        assert_eq!(&expected, parser.queue());
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
                            Token::new(Rule::block, 0, 371),
                            Token::new(Rule::statement, 0, 13),
                            Token::new(Rule::content, 0, 13),
                            Token::new(Rule::statement, 13, 35),
                            Token::new(Rule::partial, 13, 35),
                            Token::new(Rule::partial_id, 17, 32),
                            Token::new(Rule::statement, 48, 69),
                            Token::new(Rule::content, 48, 69),
                            Token::new(Rule::statement, 69, 156),
                            Token::new(Rule::section, 69, 156),
                            Token::new(Rule::path, 73, 79),
                            Token::new(Rule::identifier, 73, 79),
                            Token::new(Rule::block, 102, 144),
                            Token::new(Rule::statement, 102, 106),
                            Token::new(Rule::content, 102, 106),
                            Token::new(Rule::statement, 106, 122),
                            Token::new(Rule::variable, 106, 122),
                            Token::new(Rule::path, 109, 119),
                            Token::new(Rule::identifier, 109, 113),
                            Token::new(Rule::identifier, 114, 119),
                            Token::new(Rule::statement, 122, 144),
                            Token::new(Rule::content, 122, 144),
                            Token::new(Rule::path, 148, 154),
                            Token::new(Rule::identifier, 148, 154),
                            Token::new(Rule::statement, 173, 283),
                            Token::new(Rule::inverted, 173, 283),
                            Token::new(Rule::path, 177, 183),
                            Token::new(Rule::identifier, 177, 183),
                            Token::new(Rule::block, 245, 271),
                            Token::new(Rule::statement, 245, 271),
                            Token::new(Rule::content, 245, 271),
                            Token::new(Rule::path, 275, 281),
                            Token::new(Rule::identifier, 275, 281),
                            Token::new(Rule::statement, 296, 314),
                            Token::new(Rule::content, 296, 314),
                            Token::new(Rule::statement, 314, 336),
                            Token::new(Rule::partial, 314, 336),
                            Token::new(Rule::partial_id, 318, 333),
                            Token::new(Rule::statement, 349, 371),
                            Token::new(Rule::html, 349, 371),
                            Token::new(Rule::path, 353, 367),
                            Token::new(Rule::identifier, 353, 362),
                            Token::new(Rule::identifier, 363, 367)];
        assert_eq!(&expected, parser.queue());
    }

    #[test]
    fn invalid_section() {
        let mut parser = Rdp::new(StringInput::new("{{#one}}test{{/two}}"));
        assert!(parser.program());
        assert!(parser.end());
        match parser.tree() {
            Err(ParseError::InvalidSection(..)) => (),
            _ => panic!("Must enforce matching section paths"),
        }
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

        let list = vec![Statement::Content(String::from("<li>")),
                        Statement::Variable(Path::new(vec![String::from("name"),
                                                           String::from("first")])),
                        Statement::Content(String::from("</li>\n                "))];

        let section = Statement::Section(Path::new(vec![String::from("robots")]),
                                         Block { statements: list });

        let invblock = vec![Statement::Content(String::from("No robots\n                "))];
        let inverted = Statement::Inverted(Path::new(vec![String::from("robots")]),
                                           Block { statements: invblock });

        let program = vec![Statement::Content(String::from("\n            ")),
                           Statement::Partial(String::from("includes/header")),
                           Statement::Content(String::from("<ul>\n                ")),
                           section,
                           inverted,
                           Statement::Content(String::from("</ul>\n            ")),
                           Statement::Partial(String::from("includes/footer")),
                           Statement::Html(Path::new(vec![String::from("unescaped"),
                                                          String::from("html")]))];
        let expected = Statement::Program(Block { statements: program });

        match parser.tree() {
            Ok(tree) => assert_eq!(expected, tree),
            Err(e) => panic!("{}", e),
        }
    }
}
