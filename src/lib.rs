#![recursion_limit = "80"]

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

    fn empty() -> Self {
        Self::new(vec![])
    }

    /// Adds the statement as the first element in the block, combining it
    /// with a previous content statement if possible.
    fn prepend(&mut self, mut statement: Statement) {
        let leader = match self.statements.get_mut(0) {
            Some(first) => {
                if statement.merge(first) {
                    *first = statement;
                    None
                } else {
                    Some(statement)
                }
            }
            None => Some(statement),
        };

        if let Some(stmt) = leader {
            self.statements.insert(0, stmt);
        }
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
    Partial(String, Option<String>),
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
            Statement::Partial(ref name, _) => vec![name],
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

pub struct Padding {
    column: usize,
    text: String,
}

impl Padding {
    fn new(column: usize, text: &str) -> Self {
        Padding {
            column: column,
            text: text.into(),
        }
    }

    fn maybe(self) -> Option<String> {
        match self.text.len() {
            0 => None,
            _ => Some(self.text),
        }
    }
}

impl_rdp! {
    grammar! {
        program     = @{ block }
        block       = { statement* }
        statement   = { content | mcomment | section | variable | partial | html }
        content     = { (!(open | standalone_tag) ~ any)+ }
        variable    = !@{ open ~ path ~ close }
        html        = !@{ (["{{{"] ~ path ~ ["}}}"]) | (["{{&"] ~ path ~ close) }

        partial             = { standalone_partial | partial_tag }
        standalone_partial  = { indent ~ partial_tag ~ (terminator | eoi) }
        partial_id          = { (['a'..'z'] | ['A'..'Z'] | ['0'..'9'] | ["-"] | ["_"] | ["/"])+ }

        mcomment            = { standalone_comment | comment_tag }
        standalone_comment  = { indent ~ comment_tag ~ (terminator | eoi) }
        ctext               = { (!close ~ any)* }

        section_open_tag    = !@{ (["{{#"] | ["{{^"]) ~ path ~ close }
        section_close_tag   = !@{ ["{{/"] ~ path ~ close }
        partial_tag         = !@{ ["{{>"] ~ partial_id ~ close }
        comment_tag         = !@{ ["{{!"] ~ ctext ~ close }
        standalone_tag = {
            indent ~ (
                section_open_tag |
                section_close_tag |
                partial_tag |
                comment_tag
            ) ~ (terminator | eoi)
        }

        indent      = { ([" "] | ["\t"])* }
        stand_open  = { indent ~ sopen ~ terminator }
        stand_close = { indent ~ sclose ~ (terminator | eoi) }

        section     = { (stand_open | sopen) ~ block ~ (stand_close | sclose) }
        sopen       = !@{ (pound | caret) ~ [push(path)] ~ close }
        sclose      = !@{ ["{{/"] ~ [pop()] ~ close }

        open        = _{ ["{{"] }
        close       = _{ ["}}"] }
        pound       = { ["{{#"] }
        caret       = { ["{{^"] }

        dot         = { ["."] }
        path        = @{ dot | (identifier ~ (["."] ~ identifier)*) }
        identifier  = { (['a'..'z'] | ['A'..'Z'] | ['0'..'9'] | ["-"] | ["_"] | ["?"] | ["!"])+ }

        terminator  = { ["\r"]? ~ ["\n"] }
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
            },
            () => {
                Block::empty()
            }
        }

        _statements(&self) -> Vec<Statement> {
            (_: statement, mut head: _statement(), mut tail: _statements()) => {
                head.append(&mut tail);
                head
            },
            () => {
                Vec::new()
            }
        }

        _statement(&self) -> Vec<Statement> {
            (_: mcomment, statements: _comment()) => {
                statements
            },
            (&text: content) => {
                vec![Statement::Content(text.into())]
            },
            (_: variable, path: _path()) => {
                vec![Statement::Variable(path)]
            },
            (_: html, path: _path()) => {
                vec![Statement::Html(path)]
            },
            (_: partial, statements: _partial()) => {
                statements
            },
            (_: section, statements: _section()) => {
                statements
            }
        }

        _comment(&self) -> Vec<Statement> {
            (_: standalone_comment, padding: _indent(), ctext: _ctext()) => {
                let (text, terminator) = ctext;

                // Standalone comment consumes leading and trailing whitespace.
                if padding.column == 1 {
                    return vec![Statement::Comment(text)];
                }

                // Inline comment emits whitespace content.
                let mut statements = match padding.maybe() {
                    Some(text) => vec![Statement::Content(text)],
                    None => vec![],
                };

                statements.push(Statement::Comment(text));

                if let Some(text) = terminator {
                    statements.push(Statement::Content(text.into()));
                }

                statements
            },
            (ctext: _ctext()) => {
                let (text, _) = ctext;
                vec![Statement::Comment(text)]
            }
        }

        _indent(&self) -> Padding {
            (padding: indent) => {
                let (_, column) = self.input.line_col(padding.start);
                let text = self.input.slice(padding.start, padding.end);
                Padding::new(column, text)
            }
        }

        _ctext(&self) -> (String, Option<String>) {
            (_: comment_tag, &text: ctext, &terminate: terminator) => {
                (text.into(), Some(terminate.into()))
            },
            (_: comment_tag, &text: ctext) => {
                (text.into(), None)
            }
        }

        _partial(&self) -> Vec<Statement> {
            (_: standalone_partial, padding: _indent(), ident: _partial_id()) => {
                let (name, terminator) = ident;

                // Standalone partial consumes leading and trailing whitespace.
                if padding.column == 1 {
                    return vec![Statement::Partial(name, padding.maybe())];
                }

                // Inline partial emits whitespace content.
                let mut statements = match padding.maybe() {
                    Some(text) => vec![Statement::Content(text)],
                    None => vec![],
                };

                statements.push(Statement::Partial(name, None));

                if let Some(text) = terminator {
                    statements.push(Statement::Content(text.into()));
                }

                statements
            },
            (ident: _partial_id()) => {
                let (name, _) = ident;
                vec![Statement::Partial(name, None)]
            }
        }

        _partial_id(&self) -> (String, Option<String>) {
            (_: partial_tag, &name: partial_id, &terminate: terminator) => {
                (name.into(), Some(terminate.into()))
            },
            (_: partial_tag, &name: partial_id) => {
                (name.into(), None)
            }
        }

        _section(&self) -> Vec<Statement> {
            (opening: _section_open(), mut block: _block(), closing: _section_close()) => {
                let (leading, path, kind, terminator) = opening;

                // Inline open tag emits leading whitespace.
                let mut statements = match leading {
                    Some(text) => vec![Statement::Content(text)],
                    None => vec![],
                };

                // Inline open tag emits line terminator.
                if let Some(text) = terminator {
                    block.prepend(Statement::Content(text));
                }

                // Inline close tag emits leading whitespace.
                let (leading, terminator) = closing;
                if let Some(text) = leading {
                    block.append(Statement::Content(text));
                }

                // Emit fully formed section block.
                statements.push(match kind {
                    Rule::caret => Statement::Inverted(path, block),
                    Rule::pound => Statement::Section(path, block),
                    _ => unreachable!(),
                });

                // Inline close tag emits line terminator.
                if let Some(text) = terminator {
                    statements.push(Statement::Content(text));
                }

                statements
            }
        }

        _section_open(&self) -> (Option<String>, Path, Rule, Option<String>) {
            (_: stand_open, padding: _indent(), _: sopen, kind, path: _path(), &terminate: terminator) => {
                if padding.column == 1 {
                    (None, path, kind.rule, None)
                } else {
                    (padding.maybe(), path, kind.rule, Some(terminate.into()))
                }
            },
            (_: sopen, kind, path: _path()) => {
                (None, path, kind.rule, None)
            }
        }

        _section_close(&self) -> (Option<String>, Option<String>) {
            (_: stand_close, padding: _indent(), _: sclose, &terminate: terminator) => {
                if padding.column == 1 {
                    (None, None)
                } else {
                    (padding.maybe(), Some(terminate.into()))
                }
            },
            (_: stand_close, padding: _indent(), _sclose) => {
                if padding.column == 1 {
                    (None, None)
                } else {
                    (padding.maybe(), None)
                }
            },
            (_: sclose) => {
                (None, None)
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
    fn prepend() {
        let mut block = Block::new(vec![Statement::Comment("a".into())]);
        block.prepend(Statement::Content("b".into()));
        let expected = Block::new(vec![Statement::Content("b".into()),
                                       Statement::Comment("a".into())]);
        assert_eq!(expected, block);
    }

    #[test]
    fn prepend_and_merge() {
        let mut block = Block::new(vec![Statement::Content("a".into())]);
        block.prepend(Statement::Content("b".into()));
        assert_eq!(Block::new(vec![Statement::Content("ba".into())]), block);
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
    fn inline_section() {
        let mut parser = Rdp::new(StringInput::new("a{{#b}}c{{/b}}d"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("a".into()),
                           Statement::Section(Path::new(vec!["b".into()]),
                                              Block::new(vec![Statement::Content("c".into())])),
                           Statement::Content("d".into())];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn inverted_section() {
        let mut parser = Rdp::new(StringInput::new("a{{^b}}c{{/b}}d"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("a".into()),
                           Statement::Inverted(Path::new(vec!["b".into()]),
                                               Block::new(vec![Statement::Content("c".into())])),
                           Statement::Content("d".into())];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn empty_standalone_section() {
        let mut parser = Rdp::new(StringInput::new("\r\n{{^boolean}}\r\n{{/boolean}}\r\n"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("\r\n".into()),
                           Statement::Inverted(Path::new(vec!["boolean".into()]),
                                               Block::new(vec![]))];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn empty_inline_section() {
        let mut parser = Rdp::new(StringInput::new("{{^boolean}}{{/boolean}}"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Inverted(Path::new(vec!["boolean".into()]),
                                               Block::new(vec![]))];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn inline_section_on_standalone_line() {
        let mut parser = Rdp::new(StringInput::new("a\r\n{{#b}}c{{/b}}\nd"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("a\r\n".into()),
                           Statement::Section(Path::new(vec!["b".into()]),
                                              Block::new(vec![Statement::Content("c".into())])),
                           Statement::Content("\n".into()),
                           Statement::Content("d".into())];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn standalone_section_open_and_close_tags() {
        let mut parser = Rdp::new(StringInput::new("a\n{{#b}}\nc\n{{/b}}\r\nd"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("a\n".into()),
                           Statement::Section(Path::new(vec!["b".into()]),
                                              Block::new(vec![Statement::Content("c\n".into())])),
                           Statement::Content("d".into())];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn indented_standalone_section_open_and_close_tags() {
        let mut parser = Rdp::new(StringInput::new("a\n  {{#b}}\n    c\n  {{/b}}\r\nd"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("a\n".into()),
                           Statement::Section(Path::new(vec!["b".into()]),
                                              Block::new(vec![Statement::Content("    c\n"
                                                                  .into())])),
                           Statement::Content("d".into())];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn standalone_section_open_and_close_tags_at_eoi() {
        let mut parser = Rdp::new(StringInput::new("{{#b}}\nc\n{{/b}}"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Section(Path::new(vec!["b".into()]),
                                              Block::new(vec![Statement::Content("c\n".into())]))];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn inline_section_at_input_boundaries() {
        let mut parser = Rdp::new(StringInput::new("{{#b}}c{{/b}}"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Section(Path::new(vec!["b".into()]),
                                              Block::new(vec![Statement::Content("c".into())]))];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn inline_open_indented_standalone_close_at_eoi() {
        let mut parser = Rdp::new(StringInput::new("{{#b}}c\n  {{/b}}"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Section(Path::new(vec!["b".into()]),
                                              Block::new(vec![Statement::Content("c\n".into())]))];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn inline_open_indented_standalone_close_at_eoi_with_leading_content() {
        let mut parser = Rdp::new(StringInput::new("a{{#b}}\nc\n  {{/b}}"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("a".into()),
                           Statement::Section(Path::new(vec!["b".into()]),
                                              Block::new(vec![Statement::Content("\nc\n"
                                                                  .into())]))];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn inline_open_indented_inline_close() {
        let mut parser = Rdp::new(StringInput::new("{{#b}}c\n  {{/b}} a"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Section(Path::new(vec!["b".into()]),
                                              Block::new(vec![Statement::Content("c\n  "
                                                                  .into())])),
                           Statement::Content(" a".into())];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn inline_open_indented_inline_close_with_trailing_newline() {
        let mut parser = Rdp::new(StringInput::new("{{#b}}c\n d {{/b}}\na"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Section(Path::new(vec!["b".into()]),
                                              Block::new(vec![Statement::Content("c\n d "
                                                                  .into())])),
                           Statement::Content("\n".into()),
                           Statement::Content("a".into())];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn inline_partial() {
        let mut parser = Rdp::new(StringInput::new("a {{> b }} c"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("a ".into()),
                           Statement::Partial("b".into(), None),
                           Statement::Content(" c".into())];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn inline_partial_at_eoi() {
        let mut parser = Rdp::new(StringInput::new("a {{> b }}"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("a".into()),
                           Statement::Content(" ".into()),
                           Statement::Partial("b".into(), None)];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn inline_partial_at_eol() {
        let mut parser = Rdp::new(StringInput::new("a {{> b }}\nc"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("a".into()),
                           Statement::Content(" ".into()),
                           Statement::Partial("b".into(), None),
                           Statement::Content("\n".into()),
                           Statement::Content("c".into())];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn standalone_partial() {
        let mut parser = Rdp::new(StringInput::new("a\r\n{{> b }}\nc"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("a\r\n".into()),
                           Statement::Partial("b".into(), None),
                           Statement::Content("c".into())];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn indented_standalone_partial() {
        let mut parser = Rdp::new(StringInput::new("a\r\n  {{> b }}\nc"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("a\r\n".into()),
                           Statement::Partial("b".into(), Some("  ".into())),
                           Statement::Content("c".into())];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn standalone_partial_with_trailing_content() {
        let mut parser = Rdp::new(StringInput::new("a\r\n{{> b }}c"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("a\r\n".into()),
                           Statement::Partial("b".into(), None),
                           Statement::Content("c".into())];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn standalone_partial_at_eoi() {
        let mut parser = Rdp::new(StringInput::new("a\r\n  {{> b }}"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("a\r\n".into()),
                           Statement::Partial("b".into(), Some("  ".into()))];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn inline_comment() {
        let mut parser = Rdp::new(StringInput::new("a {{! b }} c"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("a ".into()),
                           Statement::Comment("b".into()),
                           Statement::Content(" c".into())];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn inline_comment_at_eoi() {
        let mut parser = Rdp::new(StringInput::new("a {{! b }}"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("a".into()),
                           Statement::Content(" ".into()),
                           Statement::Comment("b".into())];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn inline_comment_at_eol() {
        let mut parser = Rdp::new(StringInput::new("a {{! b }}\nc"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("a".into()),
                           Statement::Content(" ".into()),
                           Statement::Comment("b".into()),
                           Statement::Content("\n".into()),
                           Statement::Content("c".into())];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn standalone_comment() {
        let mut parser = Rdp::new(StringInput::new("a\r\n{{! b }}\nc"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("a\r\n".into()),
                           Statement::Comment("b".into()),
                           Statement::Content("c".into())];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn indented_standalone_comment() {
        let mut parser = Rdp::new(StringInput::new("a\r\n  {{! b }}\nc"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("a\r\n".into()),
                           Statement::Comment("b".into()),
                           Statement::Content("c".into())];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn standalone_comment_with_trailing_content() {
        let mut parser = Rdp::new(StringInput::new("a\r\n{{! b }}c"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("a\r\n".into()),
                           Statement::Comment("b".into()),
                           Statement::Content("c".into())];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }

    #[test]
    fn standalone_comment_at_eoi() {
        let mut parser = Rdp::new(StringInput::new("a\r\n  {{! b }}"));
        assert!(parser.program());
        assert!(parser.end());

        let program = vec![Statement::Content("a\r\n".into()),
                           Statement::Comment("b".into())];
        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
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


        let program =
            vec![Statement::Content("\n".into()),
                 Statement::Partial("includes/header".into(), Some("            ".into())),
                 Statement::Content("            <ul>\n".into()),
                 Statement::Section(Path::new(vec!["robots".into()]),
                                    Block::new(vec![Statement::Content("                    <li>".into()),
                                                    Statement::Variable(Path::new(vec!["name".into(),
                                                                                       "first".into()])),
                                                    Statement::Content("</li>\n".into())])),
                 Statement::Inverted(Path::new(vec!["robots".into()]),
                                     Block::new(vec![Statement::Comment("else clause".into()),
                                                     Statement::Content("                    No robots\n".into())])),
                 Statement::Content("            </ul>\n".into()),
                 Statement::Partial("includes/footer".into(), Some("            ".into())),
                 Statement::Content("            ".into()),
                 Statement::Html(Path::new(vec!["unescaped".into(), "html".into()])),
                 Statement::Content("\n        ".into())];

        let expected = Statement::Program(Block::new(program));
        assert_eq!(expected, parser.tree());
    }
}
