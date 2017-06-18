extern crate regex;

use regex::Regex;
use std::collections::HashSet;
use std::io::{self, Write};

use super::{Compile, Name, ParseError, Path, Statement, Template};
use self::runtime::RUNTIME;

mod runtime;

/// A program is the final result of Mustache AST to Ruby extension source
/// translation that is presented to the main compiler driver for output.
///
/// It contains all external rendering functions generated by the translator,
/// which are exposed to Ruby code after the extension is compiled.
#[derive(Debug)]
pub struct Program {
    global: Scope,
}

impl Program {
    fn new() -> Self {
        Program { global: Scope::new(Name::new("global")) }
    }

    fn merge(&mut self, scope: Scope) -> &mut Self {
        self.global.merge(scope);
        self
    }
}

impl Compile for Program {
    /// Writes the final translated source code to an output buffer.
    ///
    /// This emits fully-formed Ruby extension source code that may be input
    /// into a mkmf build process, creating a dynamically loadable shared
    /// object file.
    fn emit(&self, buf: &mut Write) -> io::Result<()> {
        // Emit runtime preamble.
        writeln!(buf, "{}", RUNTIME)?;

        // Emit string content declarations.
        for string in &self.global.strings {
            string.emit(buf)?;
        }

        writeln!(buf, "")?;

        // Emit function declarations.
        for fun in &self.global.functions {
            writeln!(buf, "{};", fun.decl)?;
        }

        writeln!(buf, "")?;

        // Emit function definitions.
        for fun in &self.global.functions {
            fun.emit(buf)?
        }

        // Emit public render function.
        let renders: Vec<_> = self.global
            .functions
            .iter()
            .filter_map(|f| f.invoke_if())
            .collect();

        writeln!(
            buf,
            r#"static VALUE render(VALUE self, VALUE name, VALUE context) {{
                   const char *ptr = StringValuePtr(name);
                   const long length = RSTRING_LEN(name);
                   const struct stack stack = {{ .data = context, .parent = NULL }};

                   struct buffer *buf = templates_get_buf(self);
                   buffer_clear(buf);

                   {}
                   else {{
                       rb_raise(rb_eArgError, "Template not found");
                   }}

                   return rb_str_new(buf->data, buf->length);
               }}"#,
            renders.join(" else ")
        )
    }
}

/// A store for functions created by the translation process of an input
/// template to source code output.
///
/// Scopes have name generators that are used in function naming, providing a
/// stable name that other scopes may rely on for partial template function
/// calls.
///
/// After each template is translated into a scope they are merged into a
/// Program's global scope for final output.
#[derive(Debug)]
struct Scope {
    name: Name,
    functions: Vec<Function>,
    strings: Vec<StaticString>,
}

impl Scope {
    fn new(name: Name) -> Self {
        Scope {
            name: name,
            functions: Vec::new(),
            strings: Vec::new(),
        }
    }

    /// Combines this scope's function definitions with another's.
    fn merge(&mut self, mut other: Scope) -> &mut Self {
        self.functions.append(&mut other.functions);
        self.strings.append(&mut other.strings);
        self
    }

    /// Advances the scope's name generator to the next unique identifier. This
    /// should be called before descending another level in the recursive
    /// tree translation process.
    fn next(&mut self) -> &mut Self {
        self.name.next();
        self
    }

    /// Adds a function to this scope.
    fn register(&mut self, fun: Function) {
        self.functions.push(fun);
    }

    /// Adds a constant string value to this scope.
    fn content(&mut self, string: StaticString) {
        self.strings.push(string);
    }

    /// Returns the template path used to generate function names in this
    /// scope (e.g. "includes/header").
    fn base_name(&self) -> String {
        self.name.base.clone()
    }
}

#[derive(Debug)]
struct StaticString {
    name: String,
    value: String,
    length: usize,
}

impl StaticString {
    /// Writes the raw content string global to the buffer.
    fn emit(&self, buf: &mut Write) -> io::Result<()> {
        writeln!(
            buf,
            "static const char *{} = \"{}\";",
            self.name,
            self.value
        )
    }
}

#[derive(Debug)]
struct Function {
    name: String,
    decl: String,
    body: Vec<String>,
    export: Option<String>,
}

impl Function {
    /// Writes the function definition to the buffer.
    fn emit(&self, buf: &mut Write) -> io::Result<()> {
        writeln!(buf, "{} {{", self.decl)?;
        for node in &self.body {
            writeln!(buf, "{}", node)?;
        }
        writeln!(buf, "}}\n")
    }

    /// Builds a conditional statement to call the function if the template
    /// name matches the function's exported name, like "includes/header".
    fn invoke_if(&self) -> Option<String> {
        if self.export.is_none() {
            return None;
        }

        let export = self.export.as_ref().unwrap();
        Some(format!(
            "if (length == {len} && strncmp(ptr, \"{path}\", {len}) == 0) {{
                 {fun}(buf, &stack);
             }}",
            len = export.len(),
            path = export,
            fun = self.name
        ))
    }
}

/// Recursively walks the AST, translating Mustache statement tree nodes into
/// the corresponding Ruby extension source code.
///
/// Sections are extracted into top-level functions paired with a function
/// call at the location the section appeared in the template. Partials are
/// similarly translated into a function call which is expected to be provided
/// by another template in the final tree.
fn transform(scope: &mut Scope, node: &Statement) -> Option<String> {
    match *node {
        Statement::Program(ref block) => {
            let id = scope.name.id();

            // Build private render function.
            let children = block
                .statements
                .iter()
                .filter_map(|stmt| transform(scope.next(), stmt))
                .collect();

            let render = Function {
                name: format!("render_{}", id),
                decl: format!(
                    "static void render_{}(struct buffer *buf, const struct stack *stack)",
                    id
                ),
                body: children,
                export: Some(scope.base_name()),
            };

            scope.register(render);
            None
        }
        Statement::Section(ref path, ref block) => {
            let children = block
                .statements
                .iter()
                .filter_map(|stmt| transform(scope.next(), stmt))
                .collect();

            let name = format!("section_{}", scope.next().name);
            let fun = Function {
                decl: format!(
                    "static void {}(struct buffer *buf, const struct stack *stack)",
                    name
                ),
                name: name,
                body: children,
                export: None,
            };

            let call = format!(
                "{{ {} section(buf, stack, &path, {}); }}",
                path_ary(path),
                fun.name
            );

            scope.register(fun);
            Some(call)
        }
        Statement::Inverted(ref path, ref block) => {
            let children = block
                .statements
                .iter()
                .filter_map(|stmt| transform(scope.next(), stmt))
                .collect();

            let name = format!("section_{}", scope.next().name);
            let fun = Function {
                decl: format!(
                    "static void {}(struct buffer *buf, const struct stack *stack)",
                    name
                ),
                name: name,
                body: children,
                export: None,
            };

            let call = format!(
                "{{ {} inverted(buf, stack, &path, {}); }}",
                path_ary(path),
                fun.name
            );

            scope.register(fun);
            Some(call)
        }
        Statement::Partial(ref name, ref padding) => {
            let name = Name::new(name);
            Some(format!("render_{}(buf, stack);", name.id()))
        }
        Statement::Comment(_) => None,
        Statement::Content(ref text) => {
            let content = clean(text);

            let string = StaticString {
                name: format!("content_{}", scope.next().name),
                value: content,
                length: text.len(),
            };

            let append = format!("buffer_append(buf, {}, {});", string.name, string.length);

            scope.content(string);
            Some(append)
        }
        Statement::Variable(ref path) => {
            let path = path_ary(path);
            Some(format!(
                "{{ {} append_value(buf, stack, &path, true); }}",
                path
            ))
        }
        Statement::Html(ref path) => {
            let path = path_ary(path);
            Some(format!(
                "{{ {} append_value(buf, stack, &path, false); }}",
                path
            ))
        }
    }
}

/// Transforms the AST of each parsed template into a source code tree
/// and links each template together into a single executable program.
pub fn link(templates: &Vec<Template>) -> Result<Program, ParseError> {
    validate(templates)?;

    let mut program = Program::new();
    templates
        .iter()
        .map(|template| {
            let mut scope = Scope::new(template.name());
            transform(&mut scope, &template.tree);
            scope
        })
        .fold(&mut program, |program, scope| program.merge(scope));

    Ok(program)
}

/// Ensures all templates may be linked together into an executable.
///
/// This method checks that all partial template paths are provided by
/// another template. For example, a `{{>include/header}}` partial invocation
/// must be provided by an `include/header.mustache` template file.
///
/// Partials can be considered function calls, so the function must be defined.
fn validate(templates: &Vec<Template>) -> Result<(), ParseError> {
    let all: HashSet<_> = templates.iter().map(|temp| &temp.name).collect();

    for template in templates {
        let names: HashSet<_> = template.tree.partials().into_iter().collect();
        let missing = &names - &all;
        if !missing.is_empty() {
            let name = missing.into_iter().next().unwrap();
            return Err(ParseError::UnknownPartial(
                name.clone(),
                template.path.clone(),
            ));
        }
    }

    Ok(())
}

/// Replaces string literal characters considered invalid inside a cstr with
/// their escaped counterparts.
fn clean(text: &str) -> String {
    let re = Regex::new(r"\\").unwrap();
    let text = re.replace_all(&text, "\\\\");

    let re = Regex::new(r"\r").unwrap();
    let text = re.replace_all(&text, "\\r");

    let re = Regex::new(r"\n").unwrap();
    let text = re.replace_all(&text, "\\n");

    let re = Regex::new(r#"["]"#).unwrap();
    re.replace_all(&text, "\\\"").into_owned()
}

/// Transforms a Mustache variable key path into the source code to build a
/// Ruby array. At runtime, each key in the array is recursively processed to
/// find the replacement text for a Mustache expression.
fn path_ary(path: &Path) -> String {
    let args = path.keys
        .iter()
        .map(|key| format!("\"{}\"", key))
        .collect::<Vec<String>>()
        .join(", ");

    format!(
        "static const struct path path = {{ .keys = {{ {} }}, .length = {} }};",
        args,
        path.keys.len()
    )
}

#[cfg(test)]
mod tests {
    use super::{link, transform, Scope};
    use super::super::{Name, ParseError, Statement, Template};
    use std::path::{Path, PathBuf};

    #[test]
    fn validates_valid_partial_reference() {
        let base = PathBuf::from("app/templates");
        let path = PathBuf::from("app/templates/machines/robots.mustache");
        let tree = Statement::Partial(String::from("machines/robot"), None);
        let master = Template::new(&base, path, tree);

        let path = PathBuf::from("app/templates/machines/robot.mustache");
        let tree = Statement::Content(String::from("hubot"));
        let detail = Template::new(&base, path, tree);

        let templates = vec![master, detail];
        match link(&templates) {
            Ok(_) => (),
            Err(e) => panic!("Must link valid partials: {}", e),
        }
    }

    #[test]
    fn validates_invalid_partial_reference() {
        let base = PathBuf::from("app/templates");
        let path = PathBuf::from("app/templates/machines/robots.mustache");
        let tree = Statement::Partial(String::from("machines/unknown"), None);

        let master = Template::new(&base, path, tree);

        let path = PathBuf::from("app/templates/machines/robot.mustache");
        let tree = Statement::Content(String::from("hubot"));
        let detail = Template::new(&base, path, tree);

        let templates = vec![master, detail];
        match link(&templates) {
            Err(ParseError::UnknownPartial(ref name, ref path)) => {
                assert_eq!("machines/unknown", name);
                assert_eq!(Path::new("app/templates/machines/robots.mustache"), path);
            }
            _ => panic!("Must enforce partial references"),
        }
    }

    #[test]
    fn transforms_tree_into_functions() {
        let text = "
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
        ";

        match Statement::parse(text) {
            Ok(tree) => {
                let mut scope = Scope::new(Name::new("machines/robot"));
                transform(&mut scope, &tree);

                // One for each section, private render, and exported template function.
                let names: Vec<_> = scope.functions.iter().map(|fun| &fun.name).collect();
                assert_eq!(
                    vec![
                        "section_machines_robot12",
                        "section_machines_robot17",
                        "render_machines_robot",
                    ],
                    names
                );

                // Single exported function name.
                let exports: Vec<_> = scope
                    .functions
                    .iter()
                    .filter_map(|fun| fun.export.as_ref())
                    .collect();
                assert_eq!(vec!["machines/robot"], exports);
            }
            Err(e) => panic!("Failed to parse tree: {}", e),
        }
    }
}
