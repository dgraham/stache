#[macro_use]
extern crate pest;

use pest::prelude::*;

impl_rdp! {
    grammar! {
        program     = { block ~ eoi }
        block       = { statement* }
        statement   = { content | variable | html | section | inverted | partial }
        content     = @{ (!open ~ any)+ }
        variable    = { open ~ path ~ close }
        html        = { ["{{{"] ~ path ~ ["}}}"]}
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
}

#[cfg(test)]
mod tests {
    use pest::prelude::*;
    use super::{Rdp, Rule};

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
}
