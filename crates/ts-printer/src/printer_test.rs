use std::cell::RefCell;
use std::rc::Rc;

use ts_ast as ast;
use ts_core as core;
use ts_parser as parser;
use ts_sourcemap as sourcemap;
use ts_tspath as tspath;

use crate::{
    EmitTextWriter, PrintHandlers, Printer, PrinterOptions, new_printer, new_text_writer,
    share_text_writer,
};

#[derive(Debug, PartialEq, Eq)]
enum SemicolonWrite {
    Punctuation,
    Trailing,
}

struct SemicolonRecordingWriter {
    inner: Box<dyn EmitTextWriter>,
    semicolon_writes: Rc<RefCell<Vec<SemicolonWrite>>>,
}

impl EmitTextWriter for SemicolonRecordingWriter {
    fn write(&mut self, s: &str) {
        self.inner.write(s)
    }

    fn write_trailing_semicolon(&mut self, text: &str) {
        if text == ";" {
            self.semicolon_writes
                .borrow_mut()
                .push(SemicolonWrite::Trailing);
        }
        self.inner.write_trailing_semicolon(text)
    }

    fn write_comment(&mut self, text: &str) {
        self.inner.write_comment(text)
    }

    fn write_keyword(&mut self, text: &str) {
        self.inner.write_keyword(text)
    }

    fn write_operator(&mut self, text: &str) {
        self.inner.write_operator(text)
    }

    fn write_punctuation(&mut self, text: &str) {
        if text == ";" {
            self.semicolon_writes
                .borrow_mut()
                .push(SemicolonWrite::Punctuation);
        }
        self.inner.write_punctuation(text)
    }

    fn write_space(&mut self, text: &str) {
        self.inner.write_space(text)
    }

    fn write_string_literal(&mut self, text: &str) {
        self.inner.write_string_literal(text)
    }

    fn write_parameter(&mut self, text: &str) {
        self.inner.write_parameter(text)
    }

    fn write_property(&mut self, text: &str) {
        self.inner.write_property(text)
    }

    fn write_symbol(&mut self, text: &str, symbol: Option<ast::SymbolHandle>) {
        self.inner.write_symbol(text, symbol)
    }

    fn write_line(&mut self) {
        self.inner.write_line()
    }

    fn write_line_force(&mut self, force: bool) {
        self.inner.write_line_force(force)
    }

    fn increase_indent(&mut self) {
        self.inner.increase_indent()
    }

    fn decrease_indent(&mut self) {
        self.inner.decrease_indent()
    }

    fn clear(&mut self) {
        self.semicolon_writes.borrow_mut().clear();
        self.inner.clear()
    }

    fn string(&self) -> String {
        self.inner.string()
    }

    fn raw_write(&mut self, s: &str) {
        self.inner.raw_write(s)
    }

    fn write_literal(&mut self, s: &str) {
        self.inner.write_literal(s)
    }

    fn get_text_pos(&self) -> i32 {
        self.inner.get_text_pos()
    }

    fn get_line(&self) -> i32 {
        self.inner.get_line()
    }

    fn get_column(&self) -> core::UTF16Offset {
        self.inner.get_column()
    }

    fn get_indent(&self) -> i32 {
        self.inner.get_indent()
    }

    fn is_at_start_of_line(&self) -> bool {
        self.inner.is_at_start_of_line()
    }

    fn has_trailing_comment(&self) -> bool {
        self.inner.has_trailing_comment()
    }

    fn has_trailing_whitespace(&self) -> bool {
        self.inner.has_trailing_whitespace()
    }
}

struct EmitCase {
    title: String,
    input: String,
    output: String,
    jsx: bool,
}

#[test]
fn emit_context_visit_parameters_moves_initializer_when_parameter_hoists_variable() {
    let mut emit_context = crate::new_emit_context();
    let old_flags = emit_context.begin_visit_parameters();
    let name = emit_context.factory.node_factory.new_identifier("x");
    let initializer = emit_context
        .factory
        .node_factory
        .new_numeric_literal("1", ast::TokenFlags::NONE);
    let parameter = emit_context.factory.node_factory.new_parameter_declaration(
        None,
        None,
        name,
        None,
        None,
        initializer,
    );
    let temp = emit_context.factory.node_factory.new_identifier("_temp");
    emit_context.add_variable_declaration(temp);

    let (parameters, changed) =
        emit_context.finish_visit_parameters(old_flags, vec![parameter], false);
    assert!(changed);
    let updated_parameter = parameters[0];
    assert!(
        emit_context
            .factory
            .node_factory
            .store()
            .initializer(updated_parameter)
            .is_none()
    );

    let body_statements = emit_context.factory.new_node_list([]);
    let body = emit_context
        .factory
        .node_factory
        .new_block(body_statements, true);
    let body = emit_context
        .finish_visit_function_body(Some(body))
        .expect("function body should be produced");
    let statements = emit_context
        .factory
        .node_factory
        .store()
        .statements(body)
        .expect("block should have statements")
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(statements.len(), 2);
    let store = emit_context.factory.node_factory.store();
    assert!(ast::is_variable_statement(store, statements[0]));
    assert_eq!(store.kind(statements[1]), ast::Kind::IfStatement);
}

fn emit_cases_from_go_source() -> Vec<EmitCase> {
    vec![
        EmitCase {
            title: "StringLiteral#1".to_string(),
            input: ";\"test\"".to_string(),
            output: ";\n\"test\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "StringLiteral#2".to_string(),
            input: ";'test'".to_string(),
            output: ";\n'test';".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NumericLiteral#1".to_string(),
            input: "0".to_string(),
            output: "0;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NumericLiteral#2".to_string(),
            input: "10_000".to_string(),
            output: "10000;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "BigIntLiteral#1".to_string(),
            input: "0n".to_string(),
            output: "0n;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "BigIntLiteral#2".to_string(),
            input: "10_000n".to_string(),
            output: "10000n;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "BooleanLiteral#1".to_string(),
            input: "true".to_string(),
            output: "true;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "BooleanLiteral#2".to_string(),
            input: "false".to_string(),
            output: "false;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NoSubstitutionTemplateLiteral".to_string(),
            input: "``".to_string(),
            output: "``;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NoSubstitutionTemplateLiteral#2".to_string(),
            input: "`\n`".to_string(),
            output: "`\n`;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "RegularExpressionLiteral#1".to_string(),
            input: "/a/".to_string(),
            output: "/a/;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "RegularExpressionLiteral#2".to_string(),
            input: "/a/g".to_string(),
            output: "/a/g;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NullLiteral".to_string(),
            input: "null".to_string(),
            output: "null;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ThisExpression".to_string(),
            input: "this".to_string(),
            output: "this;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "SuperExpression".to_string(),
            input: "super()".to_string(),
            output: "super();".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportExpression".to_string(),
            input: "import()".to_string(),
            output: "import();".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyAccess#1".to_string(),
            input: "a.b".to_string(),
            output: "a.b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyAccess#2".to_string(),
            input: "a.#b".to_string(),
            output: "a.#b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyAccess#3".to_string(),
            input: "a?.b".to_string(),
            output: "a?.b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyAccess#4".to_string(),
            input: "a?.b.c".to_string(),
            output: "a?.b.c;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyAccess#5".to_string(),
            input: "1..b".to_string(),
            output: "1..b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyAccess#6".to_string(),
            input: "1.0.b".to_string(),
            output: "1.0.b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyAccess#7".to_string(),
            input: "0x1.b".to_string(),
            output: "0x1.b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyAccess#8".to_string(),
            input: "0b1.b".to_string(),
            output: "0b1.b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyAccess#9".to_string(),
            input: "0o1.b".to_string(),
            output: "0o1.b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyAccess#10".to_string(),
            input: "10e1.b".to_string(),
            output: "10e1.b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyAccess#11".to_string(),
            input: "10E1.b".to_string(),
            output: "10E1.b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyAccess#12".to_string(),
            input: "a.b?.c".to_string(),
            output: "a.b?.c;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyAccess#13".to_string(),
            input: "a\n.b".to_string(),
            output: "a\n    .b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyAccess#14".to_string(),
            input: "a.\nb".to_string(),
            output: "a.\n    b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ElementAccess#1".to_string(),
            input: "a[b]".to_string(),
            output: "a[b];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ElementAccess#2".to_string(),
            input: "a?.[b]".to_string(),
            output: "a?.[b];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ElementAccess#3".to_string(),
            input: "a?.[b].c".to_string(),
            output: "a?.[b].c;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "CallExpression#1".to_string(),
            input: "a()".to_string(),
            output: "a();".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "CallExpression#2".to_string(),
            input: "a<T>()".to_string(),
            output: "a<T>();".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "CallExpression#3".to_string(),
            input: "a(b)".to_string(),
            output: "a(b);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "CallExpression#4".to_string(),
            input: "a<T>(b)".to_string(),
            output: "a<T>(b);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "CallExpression#5".to_string(),
            input: "a(b).c".to_string(),
            output: "a(b).c;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "CallExpression#6".to_string(),
            input: "a<T>(b).c".to_string(),
            output: "a<T>(b).c;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "CallExpression#7".to_string(),
            input: "a?.(b)".to_string(),
            output: "a?.(b);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "CallExpression#8".to_string(),
            input: "a?.<T>(b)".to_string(),
            output: "a?.<T>(b);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "CallExpression#9".to_string(),
            input: "a?.(b).c".to_string(),
            output: "a?.(b).c;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "CallExpression#10".to_string(),
            input: "a?.<T>(b).c".to_string(),
            output: "a?.<T>(b).c;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "CallExpression#11".to_string(),
            input: "a<T, U>()".to_string(),
            output: "a<T, U>();".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "CallExpression#13".to_string(),
            input: "a?.b()".to_string(),
            output: "a?.b();".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NewExpression#1".to_string(),
            input: "new a".to_string(),
            output: "new a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NewExpression#2".to_string(),
            input: "new a.b".to_string(),
            output: "new a.b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NewExpression#3".to_string(),
            input: "new a()".to_string(),
            output: "new a();".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NewExpression#4".to_string(),
            input: "new a.b()".to_string(),
            output: "new a.b();".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NewExpression#5".to_string(),
            input: "new a<T>()".to_string(),
            output: "new a<T>();".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NewExpression#6".to_string(),
            input: "new a.b<T>()".to_string(),
            output: "new a.b<T>();".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NewExpression#7".to_string(),
            input: "new a(b)".to_string(),
            output: "new a(b);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NewExpression#8".to_string(),
            input: "new a.b(c)".to_string(),
            output: "new a.b(c);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NewExpression#9".to_string(),
            input: "new a<T>(b)".to_string(),
            output: "new a<T>(b);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NewExpression#10".to_string(),
            input: "new a.b<T>(c)".to_string(),
            output: "new a.b<T>(c);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NewExpression#11".to_string(),
            input: "new a(b).c".to_string(),
            output: "new a(b).c;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NewExpression#12".to_string(),
            input: "new a<T>(b).c".to_string(),
            output: "new a<T>(b).c;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TaggedTemplateExpression#1".to_string(),
            input: "tag``".to_string(),
            output: "tag ``;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TaggedTemplateExpression#2".to_string(),
            input: "tag<T>``".to_string(),
            output: "tag<T> ``;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeAssertionExpression#1".to_string(),
            input: "<T>a".to_string(),
            output: "<T>a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "FunctionExpression#1".to_string(),
            input: "(function(){})".to_string(),
            output: "(function () { });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "FunctionExpression#2".to_string(),
            input: "(function f(){})".to_string(),
            output: "(function f() { });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "FunctionExpression#3".to_string(),
            input: "(function*f(){})".to_string(),
            output: "(function* f() { });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "FunctionExpression#4".to_string(),
            input: "(async function f(){})".to_string(),
            output: "(async function f() { });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "FunctionExpression#5".to_string(),
            input: "(async function*f(){})".to_string(),
            output: "(async function* f() { });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "FunctionExpression#6".to_string(),
            input: "(function<T>(){})".to_string(),
            output: "(function <T>() { });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "FunctionExpression#7".to_string(),
            input: "(function(a){})".to_string(),
            output: "(function (a) { });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "FunctionExpression#8".to_string(),
            input: "(function():T{})".to_string(),
            output: "(function (): T { });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrowFunction#1".to_string(),
            input: "a=>{}".to_string(),
            output: "a => { };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrowFunction#2".to_string(),
            input: "()=>{}".to_string(),
            output: "() => { };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrowFunction#3".to_string(),
            input: "(a)=>{}".to_string(),
            output: "(a) => { };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrowFunction#4".to_string(),
            input: "<T>(a)=>{}".to_string(),
            output: "<T>(a) => { };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrowFunction#5".to_string(),
            input: "async a=>{}".to_string(),
            output: "async (a) => { };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrowFunction#6".to_string(),
            input: "async()=>{}".to_string(),
            output: "async () => { };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrowFunction#7".to_string(),
            input: "async<T>()=>{}".to_string(),
            output: "async <T>() => { };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrowFunction#8".to_string(),
            input: "():T=>{}".to_string(),
            output: "(): T => { };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrowFunction#9".to_string(),
            input: "()=>a".to_string(),
            output: "() => a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "DeleteExpression".to_string(),
            input: "delete a".to_string(),
            output: "delete a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeOfExpression".to_string(),
            input: "typeof a".to_string(),
            output: "typeof a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "VoidExpression".to_string(),
            input: "void a".to_string(),
            output: "void a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "AwaitExpression".to_string(),
            input: "await a".to_string(),
            output: "await a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PrefixUnaryExpression#1".to_string(),
            input: "+a".to_string(),
            output: "+a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PrefixUnaryExpression#2".to_string(),
            input: "++a".to_string(),
            output: "++a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PrefixUnaryExpression#3".to_string(),
            input: "+ +a".to_string(),
            output: "+ +a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PrefixUnaryExpression#4".to_string(),
            input: "+ ++a".to_string(),
            output: "+ ++a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PrefixUnaryExpression#5".to_string(),
            input: "-a".to_string(),
            output: "-a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PrefixUnaryExpression#6".to_string(),
            input: "--a".to_string(),
            output: "--a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PrefixUnaryExpression#7".to_string(),
            input: "- -a".to_string(),
            output: "- -a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PrefixUnaryExpression#8".to_string(),
            input: "- --a".to_string(),
            output: "- --a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PrefixUnaryExpression#9".to_string(),
            input: "+-a".to_string(),
            output: "+-a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PrefixUnaryExpression#10".to_string(),
            input: "+--a".to_string(),
            output: "+--a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PrefixUnaryExpression#11".to_string(),
            input: "-+a".to_string(),
            output: "-+a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PrefixUnaryExpression#12".to_string(),
            input: "-++a".to_string(),
            output: "-++a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PrefixUnaryExpression#13".to_string(),
            input: "~a".to_string(),
            output: "~a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PrefixUnaryExpression#14".to_string(),
            input: "!a".to_string(),
            output: "!a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PostfixUnaryExpression#1".to_string(),
            input: "a++".to_string(),
            output: "a++;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PostfixUnaryExpression#2".to_string(),
            input: "a--".to_string(),
            output: "a--;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "BinaryExpression#1".to_string(),
            input: "a,b".to_string(),
            output: "a, b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "BinaryExpression#2".to_string(),
            input: "a+b".to_string(),
            output: "a + b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "BinaryExpression#3".to_string(),
            input: "a**b".to_string(),
            output: "a ** b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "BinaryExpression#4".to_string(),
            input: "a instanceof b".to_string(),
            output: "a instanceof b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "BinaryExpression#5".to_string(),
            input: "a in b".to_string(),
            output: "a in b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "BinaryExpression#6".to_string(),
            input: "a\n&& b".to_string(),
            output: "a\n    && b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "BinaryExpression#7".to_string(),
            input: "a &&\nb".to_string(),
            output: "a &&\n    b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ConditionalExpression#1".to_string(),
            input: "a?b:c".to_string(),
            output: "a ? b : c;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ConditionalExpression#2".to_string(),
            input: "a\n?b:c".to_string(),
            output: "a\n    ? b : c;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ConditionalExpression#3".to_string(),
            input: "a?\nb:c".to_string(),
            output: "a ?\n    b : c;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ConditionalExpression#4".to_string(),
            input: "a?b\n:c".to_string(),
            output: "a ? b\n    : c;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ConditionalExpression#5".to_string(),
            input: "a?b:\nc".to_string(),
            output: "a ? b :\n    c;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TemplateExpression#1".to_string(),
            input: "`a${b}c`".to_string(),
            output: "`a${b}c`;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TemplateExpression#2".to_string(),
            input: "`a${b}c${d}e`".to_string(),
            output: "`a${b}c${d}e`;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "YieldExpression#1".to_string(),
            input: "(function*() { yield })".to_string(),
            output: "(function* () { yield; });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "YieldExpression#2".to_string(),
            input: "(function*() { yield a })".to_string(),
            output: "(function* () { yield a; });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "YieldExpression#3".to_string(),
            input: "(function*() { yield*a })".to_string(),
            output: "(function* () { yield* a; });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "SpreadElement".to_string(),
            input: "[...a]".to_string(),
            output: "[...a];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassExpression#1".to_string(),
            input: "(class {})".to_string(),
            output: "(class {\n});".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassExpression#2".to_string(),
            input: "(class a {})".to_string(),
            output: "(class a {\n});".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassExpression#3".to_string(),
            input: "(class<T>{})".to_string(),
            output: "(class<T> {\n});".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassExpression#4".to_string(),
            input: "(class a<T>{})".to_string(),
            output: "(class a<T> {\n});".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassExpression#5".to_string(),
            input: "(class extends b {})".to_string(),
            output: "(class extends b {\n});".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassExpression#6".to_string(),
            input: "(class a extends b {})".to_string(),
            output: "(class a extends b {\n});".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassExpression#7".to_string(),
            input: "(class implements b {})".to_string(),
            output: "(class implements b {\n});".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassExpression#8".to_string(),
            input: "(class a implements b {})".to_string(),
            output: "(class a implements b {\n});".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassExpression#9".to_string(),
            input: "(class implements b, c {})".to_string(),
            output: "(class implements b, c {\n});".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassExpression#10".to_string(),
            input: "(class a implements b, c {})".to_string(),
            output: "(class a implements b, c {\n});".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassExpression#11".to_string(),
            input: "(class extends b implements c, d {})".to_string(),
            output: "(class extends b implements c, d {\n});".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassExpression#12".to_string(),
            input: "(class a extends b implements c, d {})".to_string(),
            output: "(class a extends b implements c, d {\n});".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassExpression#13".to_string(),
            input: "(@a class {})".to_string(),
            output: "(\n@a\nclass {\n});".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "OmittedExpression".to_string(),
            input: "[,]".to_string(),
            output: "[,];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExpressionWithTypeArguments".to_string(),
            input: "a<T>".to_string(),
            output: "a<T>;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "AsExpression".to_string(),
            input: "a as T".to_string(),
            output: "a as T;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "SatisfiesExpression".to_string(),
            input: "a satisfies T".to_string(),
            output: "a satisfies T;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NonNullExpression".to_string(),
            input: "a!".to_string(),
            output: "a!;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MetaProperty#1".to_string(),
            input: "new.target".to_string(),
            output: "new.target;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MetaProperty#2".to_string(),
            input: "import.meta".to_string(),
            output: "import.meta;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrayLiteralExpression#1".to_string(),
            input: "[]".to_string(),
            output: "[];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrayLiteralExpression#2".to_string(),
            input: "[a]".to_string(),
            output: "[a];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrayLiteralExpression#3".to_string(),
            input: "[a,]".to_string(),
            output: "[a,];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrayLiteralExpression#4".to_string(),
            input: "[,a]".to_string(),
            output: "[, a];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrayLiteralExpression#5".to_string(),
            input: "[...a]".to_string(),
            output: "[...a];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrayLiteralExpression#6".to_string(),
            input: "const array = [/* comment */];".to_string(),
            output: "const array = [ /* comment */];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ObjectLiteralExpression#1".to_string(),
            input: "({})".to_string(),
            output: "({});".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ObjectLiteralExpression#2".to_string(),
            input: "({a,})".to_string(),
            output: "({ a, });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ShorthandPropertyAssignment".to_string(),
            input: "({a})".to_string(),
            output: "({ a });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyAssignment".to_string(),
            input: "({a:b})".to_string(),
            output: "({ a: b });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "SpreadAssignment".to_string(),
            input: "({...a})".to_string(),
            output: "({ ...a });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "Block".to_string(),
            input: "{}".to_string(),
            output: "{ }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "VariableStatement#1".to_string(),
            input: "var a".to_string(),
            output: "var a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "VariableStatement#2".to_string(),
            input: "let a".to_string(),
            output: "let a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "VariableStatement#3".to_string(),
            input: "const a = b".to_string(),
            output: "const a = b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "VariableStatement#4".to_string(),
            input: "using a = b".to_string(),
            output: "using a = b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "VariableStatement#5".to_string(),
            input: "await using a = b".to_string(),
            output: "await using a = b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "EmptyStatement".to_string(),
            input: ";".to_string(),
            output: ";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "IfStatement#1".to_string(),
            input: "if(a);".to_string(),
            output: "if (a)\n    ;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "IfStatement#2".to_string(),
            input: "if(a);else;".to_string(),
            output: "if (a)\n    ;\nelse\n    ;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "IfStatement#3".to_string(),
            input: "if(a);else{}".to_string(),
            output: "if (a)\n    ;\nelse { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "IfStatement#4".to_string(),
            input: "if(a);else if(b);".to_string(),
            output: "if (a)\n    ;\nelse if (b)\n    ;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "IfStatement#5".to_string(),
            input: "if(a);else if(b) {}".to_string(),
            output: "if (a)\n    ;\nelse if (b) { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "IfStatement#6".to_string(),
            input: "if(a) {}".to_string(),
            output: "if (a) { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "IfStatement#7".to_string(),
            input: "if(a) {} else;".to_string(),
            output: "if (a) { }\nelse\n    ;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "IfStatement#8".to_string(),
            input: "if(a) {} else {}".to_string(),
            output: "if (a) { }\nelse { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "IfStatement#9".to_string(),
            input: "if(a) {} else if(b);".to_string(),
            output: "if (a) { }\nelse if (b)\n    ;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "IfStatement#10".to_string(),
            input: "if(a) {} else if(b){}".to_string(),
            output: "if (a) { }\nelse if (b) { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "DoStatement#1".to_string(),
            input: "do;while(a);".to_string(),
            output: "do\n    ;\nwhile (a);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "DoStatement#2".to_string(),
            input: "do {} while(a);".to_string(),
            output: "do { } while (a);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "WhileStatement#1".to_string(),
            input: "while(a);".to_string(),
            output: "while (a)\n    ;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "WhileStatement#2".to_string(),
            input: "while(a) {}".to_string(),
            output: "while (a) { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ForStatement#1".to_string(),
            input: "for(;;);".to_string(),
            output: "for (;;)\n    ;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ForStatement#2".to_string(),
            input: "for(a;;);".to_string(),
            output: "for (a;;)\n    ;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ForStatement#3".to_string(),
            input: "for(var a;;);".to_string(),
            output: "for (var a;;)\n    ;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ForStatement#4".to_string(),
            input: "for(;a;);".to_string(),
            output: "for (; a;)\n    ;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ForStatement#5".to_string(),
            input: "for(;;a);".to_string(),
            output: "for (;; a)\n    ;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ForStatement#6".to_string(),
            input: "for(;;){}".to_string(),
            output: "for (;;) { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ForInStatement#1".to_string(),
            input: "for(a in b);".to_string(),
            output: "for (a in b)\n    ;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ForInStatement#2".to_string(),
            input: "for(var a in b);".to_string(),
            output: "for (var a in b)\n    ;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ForInStatement#3".to_string(),
            input: "for(a in b){}".to_string(),
            output: "for (a in b) { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ForOfStatement#1".to_string(),
            input: "for(a of b);".to_string(),
            output: "for (a of b)\n    ;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ForOfStatement#2".to_string(),
            input: "for(var a of b);".to_string(),
            output: "for (var a of b)\n    ;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ForOfStatement#3".to_string(),
            input: "for(a of b){}".to_string(),
            output: "for (a of b) { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ForOfStatement#4".to_string(),
            input: "for await(a of b);".to_string(),
            output: "for await (a of b)\n    ;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ForOfStatement#5".to_string(),
            input: "for await(var a of b);".to_string(),
            output: "for await (var a of b)\n    ;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ForOfStatement#6".to_string(),
            input: "for await(a of b){}".to_string(),
            output: "for await (a of b) { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ContinueStatement#1".to_string(),
            input: "continue".to_string(),
            output: "continue;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ContinueStatement#2".to_string(),
            input: "continue a".to_string(),
            output: "continue a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "BreakStatement#1".to_string(),
            input: "break".to_string(),
            output: "break;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "BreakStatement#2".to_string(),
            input: "break a".to_string(),
            output: "break a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ReturnStatement#1".to_string(),
            input: "return".to_string(),
            output: "return;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ReturnStatement#2".to_string(),
            input: "return a".to_string(),
            output: "return a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "WithStatement#1".to_string(),
            input: "with(a);".to_string(),
            output: "with (a)\n    ;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "WithStatement#2".to_string(),
            input: "with(a){}".to_string(),
            output: "with (a) { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "SwitchStatement".to_string(),
            input: "switch (a) {}".to_string(),
            output: "switch (a) {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "CaseClause#1".to_string(),
            input: "switch (a) {case b:}".to_string(),
            output: "switch (a) {\n    case b:\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "CaseClause#2".to_string(),
            input: "switch (a) {case b:;}".to_string(),
            output: "switch (a) {\n    case b: ;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "DefaultClause#1".to_string(),
            input: "switch (a) {default:}".to_string(),
            output: "switch (a) {\n    default:\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "DefaultClause#2".to_string(),
            input: "switch (a) {default:;}".to_string(),
            output: "switch (a) {\n    default: ;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "LabeledStatement".to_string(),
            input: "a:;".to_string(),
            output: "a: ;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ThrowStatement".to_string(),
            input: "throw a".to_string(),
            output: "throw a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TryStatement#1".to_string(),
            input: "try {} catch {}".to_string(),
            output: "try { }\ncatch { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TryStatement#2".to_string(),
            input: "try {} finally {}".to_string(),
            output: "try { }\nfinally { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TryStatement#3".to_string(),
            input: "try {} catch {} finally {}".to_string(),
            output: "try { }\ncatch { }\nfinally { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "DebuggerStatement".to_string(),
            input: "debugger".to_string(),
            output: "debugger;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "FunctionDeclaration#1".to_string(),
            input: "export default function(){}".to_string(),
            output: "export default function () { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "FunctionDeclaration#2".to_string(),
            input: "function f(){}".to_string(),
            output: "function f() { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "FunctionDeclaration#3".to_string(),
            input: "function*f(){}".to_string(),
            output: "function* f() { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "FunctionDeclaration#4".to_string(),
            input: "async function f(){}".to_string(),
            output: "async function f() { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "FunctionDeclaration#5".to_string(),
            input: "async function*f(){}".to_string(),
            output: "async function* f() { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "FunctionDeclaration#6".to_string(),
            input: "function f<T>(){}".to_string(),
            output: "function f<T>() { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "FunctionDeclaration#7".to_string(),
            input: "function f(a){}".to_string(),
            output: "function f(a) { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "FunctionDeclaration#8".to_string(),
            input: "function f():T{}".to_string(),
            output: "function f(): T { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "FunctionDeclaration#9".to_string(),
            input: "function f();".to_string(),
            output: "function f();".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassDeclaration#1".to_string(),
            input: "class a {}".to_string(),
            output: "class a {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassDeclaration#2".to_string(),
            input: "class a<T>{}".to_string(),
            output: "class a<T> {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassDeclaration#3".to_string(),
            input: "class a extends b {}".to_string(),
            output: "class a extends b {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassDeclaration#4".to_string(),
            input: "class a implements b {}".to_string(),
            output: "class a implements b {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassDeclaration#5".to_string(),
            input: "class a implements b, c {}".to_string(),
            output: "class a implements b, c {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassDeclaration#6".to_string(),
            input: "class a extends b implements c, d {}".to_string(),
            output: "class a extends b implements c, d {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassDeclaration#7".to_string(),
            input: "export default class {}".to_string(),
            output: "export default class {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassDeclaration#8".to_string(),
            input: "export default class<T>{}".to_string(),
            output: "export default class<T> {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassDeclaration#9".to_string(),
            input: "export default class extends b {}".to_string(),
            output: "export default class extends b {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassDeclaration#10".to_string(),
            input: "export default class implements b {}".to_string(),
            output: "export default class implements b {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassDeclaration#11".to_string(),
            input: "export default class implements b, c {}".to_string(),
            output: "export default class implements b, c {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassDeclaration#12".to_string(),
            input: "export default class extends b implements c, d {}".to_string(),
            output: "export default class extends b implements c, d {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassDeclaration#13".to_string(),
            input: "@a class b {}".to_string(),
            output: "@a\nclass b {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassDeclaration#14".to_string(),
            input: "@a export class b {}".to_string(),
            output: "@a\nexport class b {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassDeclaration#15".to_string(),
            input: "export @a class b {}".to_string(),
            output: "export \n@a\nclass b {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "InterfaceDeclaration#1".to_string(),
            input: "interface a {}".to_string(),
            output: "interface a {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "InterfaceDeclaration#2".to_string(),
            input: "interface a<T>{}".to_string(),
            output: "interface a<T> {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "InterfaceDeclaration#3".to_string(),
            input: "interface a extends b {}".to_string(),
            output: "interface a extends b {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "InterfaceDeclaration#4".to_string(),
            input: "interface a extends b, c {}".to_string(),
            output: "interface a extends b, c {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeAliasDeclaration#1".to_string(),
            input: "type a = b".to_string(),
            output: "type a = b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeAliasDeclaration#2".to_string(),
            input: "type a<T> = b".to_string(),
            output: "type a<T> = b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "EnumDeclaration#1".to_string(),
            input: "enum a{}".to_string(),
            output: "enum a {\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "EnumDeclaration#2".to_string(),
            input: "enum a{b}".to_string(),
            output: "enum a {\n    b\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "EnumDeclaration#3".to_string(),
            input: "enum a{b=c}".to_string(),
            output: "enum a {\n    b = c\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ModuleDeclaration#1".to_string(),
            input: "module a{}".to_string(),
            output: "module a { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ModuleDeclaration#2".to_string(),
            input: "module a.b{}".to_string(),
            output: "module a.b { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ModuleDeclaration#3".to_string(),
            input: "module \"a\";".to_string(),
            output: "module \"a\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ModuleDeclaration#4".to_string(),
            input: "module \"a\"{}".to_string(),
            output: "module \"a\" { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ModuleDeclaration#5".to_string(),
            input: "namespace a{}".to_string(),
            output: "namespace a { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ModuleDeclaration#6".to_string(),
            input: "namespace a.b{}".to_string(),
            output: "namespace a.b { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ModuleDeclaration#7".to_string(),
            input: "global;".to_string(),
            output: "global;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ModuleDeclaration#8".to_string(),
            input: "global{}".to_string(),
            output: "global { }".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportEqualsDeclaration#1".to_string(),
            input: "import a = b".to_string(),
            output: "import a = b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportEqualsDeclaration#2".to_string(),
            input: "import a = b.c".to_string(),
            output: "import a = b.c;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportEqualsDeclaration#3".to_string(),
            input: "import a = require(\"b\")".to_string(),
            output: "import a = require(\"b\");".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportEqualsDeclaration#4".to_string(),
            input: "export import a = b".to_string(),
            output: "export import a = b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportEqualsDeclaration#5".to_string(),
            input: "export import a = require(\"b\")".to_string(),
            output: "export import a = require(\"b\");".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportEqualsDeclaration#6".to_string(),
            input: "import type a = b".to_string(),
            output: "import type a = b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportEqualsDeclaration#7".to_string(),
            input: "import type a = b.c".to_string(),
            output: "import type a = b.c;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportEqualsDeclaration#8".to_string(),
            input: "import type a = require(\"b\")".to_string(),
            output: "import type a = require(\"b\");".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportDeclaration#1".to_string(),
            input: "import \"a\"".to_string(),
            output: "import \"a\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportDeclaration#2".to_string(),
            input: "import a from \"b\"".to_string(),
            output: "import a from \"b\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportDeclaration#3".to_string(),
            input: "import type a from \"b\"".to_string(),
            output: "import type a from \"b\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportDeclaration#4".to_string(),
            input: "import * as a from \"b\"".to_string(),
            output: "import * as a from \"b\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportDeclaration#5".to_string(),
            input: "import type * as a from \"b\"".to_string(),
            output: "import type * as a from \"b\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportDeclaration#6".to_string(),
            input: "import {} from \"b\"".to_string(),
            output: "import {} from \"b\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportDeclaration#7".to_string(),
            input: "import type {} from \"b\"".to_string(),
            output: "import type {} from \"b\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportDeclaration#8".to_string(),
            input: "import { a } from \"b\"".to_string(),
            output: "import { a } from \"b\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportDeclaration#9".to_string(),
            input: "import type { a } from \"b\"".to_string(),
            output: "import type { a } from \"b\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportDeclaration#8".to_string(),
            input: "import { a as b } from \"c\"".to_string(),
            output: "import { a as b } from \"c\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportDeclaration#9".to_string(),
            input: "import type { a as b } from \"c\"".to_string(),
            output: "import type { a as b } from \"c\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportDeclaration#10".to_string(),
            input: "import { \"a\" as b } from \"c\"".to_string(),
            output: "import { \"a\" as b } from \"c\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportDeclaration#11".to_string(),
            input: "import type { \"a\" as b } from \"c\"".to_string(),
            output: "import type { \"a\" as b } from \"c\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportDeclaration#12".to_string(),
            input: "import a, {} from \"b\"".to_string(),
            output: "import a, {} from \"b\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportDeclaration#13".to_string(),
            input: "import a, * as b from \"c\"".to_string(),
            output: "import a, * as b from \"c\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportDeclaration#14".to_string(),
            input: "import {} from \"a\" with {}".to_string(),
            output: "import {} from \"a\" with {};".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportDeclaration#15".to_string(),
            input: "import {} from \"a\" with { b: \"c\" }".to_string(),
            output: "import {} from \"a\" with { b: \"c\" };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportDeclaration#16".to_string(),
            input: "import {} from \"a\" with { \"b\": \"c\" }".to_string(),
            output: "import {} from \"a\" with { \"b\": \"c\" };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportAssignment#1".to_string(),
            input: "export = a".to_string(),
            output: "export = a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportAssignment#2".to_string(),
            input: "export default a".to_string(),
            output: "export default a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NamespaceExportDeclaration".to_string(),
            input: "export as namespace a".to_string(),
            output: "export as namespace a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#1".to_string(),
            input: "export * from \"a\"".to_string(),
            output: "export * from \"a\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#2".to_string(),
            input: "export type * from \"a\"".to_string(),
            output: "export type * from \"a\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#3".to_string(),
            input: "export * as a from \"b\"".to_string(),
            output: "export * as a from \"b\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#4".to_string(),
            input: "export type * as a from \"b\"".to_string(),
            output: "export type * as a from \"b\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#5".to_string(),
            input: "export { } from \"a\"".to_string(),
            output: "export {} from \"a\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#6".to_string(),
            input: "export type { } from \"a\"".to_string(),
            output: "export type {} from \"a\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#7".to_string(),
            input: "export { a } from \"b\"".to_string(),
            output: "export { a } from \"b\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#8".to_string(),
            input: "export { type a } from \"b\"".to_string(),
            output: "export { type a } from \"b\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#9".to_string(),
            input: "export type { a } from \"b\"".to_string(),
            output: "export type { a } from \"b\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#10".to_string(),
            input: "export { a as b } from \"c\"".to_string(),
            output: "export { a as b } from \"c\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#11".to_string(),
            input: "export { type a as b } from \"c\"".to_string(),
            output: "export { type a as b } from \"c\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#12".to_string(),
            input: "export type { a as b } from \"c\"".to_string(),
            output: "export type { a as b } from \"c\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#13".to_string(),
            input: "export { a as \"b\" } from \"c\"".to_string(),
            output: "export { a as \"b\" } from \"c\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#14".to_string(),
            input: "export { type a as \"b\" } from \"c\"".to_string(),
            output: "export { type a as \"b\" } from \"c\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#15".to_string(),
            input: "export type { a as \"b\" } from \"c\"".to_string(),
            output: "export type { a as \"b\" } from \"c\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#16".to_string(),
            input: "export { \"a\" } from \"b\"".to_string(),
            output: "export { \"a\" } from \"b\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#17".to_string(),
            input: "export { type \"a\" } from \"b\"".to_string(),
            output: "export { type \"a\" } from \"b\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#18".to_string(),
            input: "export type { \"a\" } from \"b\"".to_string(),
            output: "export type { \"a\" } from \"b\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#19".to_string(),
            input: "export { \"a\" as b } from \"c\"".to_string(),
            output: "export { \"a\" as b } from \"c\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#20".to_string(),
            input: "export { type \"a\" as b } from \"c\"".to_string(),
            output: "export { type \"a\" as b } from \"c\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#21".to_string(),
            input: "export type { \"a\" as b } from \"c\"".to_string(),
            output: "export type { \"a\" as b } from \"c\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#22".to_string(),
            input: "export { \"a\" as \"b\" } from \"c\"".to_string(),
            output: "export { \"a\" as \"b\" } from \"c\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#23".to_string(),
            input: "export { type \"a\" as \"b\" } from \"c\"".to_string(),
            output: "export { type \"a\" as \"b\" } from \"c\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#24".to_string(),
            input: "export type { \"a\" as \"b\" } from \"c\"".to_string(),
            output: "export type { \"a\" as \"b\" } from \"c\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#25".to_string(),
            input: "export { }".to_string(),
            output: "export {};".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#26".to_string(),
            input: "export type { }".to_string(),
            output: "export type {};".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#27".to_string(),
            input: "export { a }".to_string(),
            output: "export { a };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#28".to_string(),
            input: "export { type a }".to_string(),
            output: "export { type a };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#29".to_string(),
            input: "export type { a }".to_string(),
            output: "export type { a };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#30".to_string(),
            input: "export { a as b }".to_string(),
            output: "export { a as b };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#31".to_string(),
            input: "export { type a as b }".to_string(),
            output: "export { type a as b };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#32".to_string(),
            input: "export type { a as b }".to_string(),
            output: "export type { a as b };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#33".to_string(),
            input: "export { a as \"b\" }".to_string(),
            output: "export { a as \"b\" };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#34".to_string(),
            input: "export { type a as \"b\" }".to_string(),
            output: "export { type a as \"b\" };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#35".to_string(),
            input: "export type { a as \"b\" }".to_string(),
            output: "export type { a as \"b\" };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#36".to_string(),
            input: "export {} from \"a\" with {}".to_string(),
            output: "export {} from \"a\" with {};".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#37".to_string(),
            input: "export {} from \"a\" with { b: \"c\" }".to_string(),
            output: "export {} from \"a\" with { b: \"c\" };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ExportDeclaration#38".to_string(),
            input: "export {} from \"a\" with { \"b\": \"c\" }".to_string(),
            output: "export {} from \"a\" with { \"b\": \"c\" };".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "KeywordTypeNode#1".to_string(),
            input: "type T = any".to_string(),
            output: "type T = any;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "KeywordTypeNode#2".to_string(),
            input: "type T = unknown".to_string(),
            output: "type T = unknown;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "KeywordTypeNode#3".to_string(),
            input: "type T = never".to_string(),
            output: "type T = never;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "KeywordTypeNode#4".to_string(),
            input: "type T = void".to_string(),
            output: "type T = void;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "KeywordTypeNode#5".to_string(),
            input: "type T = undefined".to_string(),
            output: "type T = undefined;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "KeywordTypeNode#6".to_string(),
            input: "type T = null".to_string(),
            output: "type T = null;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "KeywordTypeNode#7".to_string(),
            input: "type T = object".to_string(),
            output: "type T = object;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "KeywordTypeNode#8".to_string(),
            input: "type T = string".to_string(),
            output: "type T = string;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "KeywordTypeNode#9".to_string(),
            input: "type T = symbol".to_string(),
            output: "type T = symbol;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "KeywordTypeNode#10".to_string(),
            input: "type T = number".to_string(),
            output: "type T = number;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "KeywordTypeNode#11".to_string(),
            input: "type T = bigint".to_string(),
            output: "type T = bigint;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "KeywordTypeNode#12".to_string(),
            input: "type T = boolean".to_string(),
            output: "type T = boolean;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "KeywordTypeNode#13".to_string(),
            input: "type T = intrinsic".to_string(),
            output: "type T = intrinsic;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypePredicateNode#1".to_string(),
            input: "function f(): asserts a".to_string(),
            output: "function f(): asserts a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypePredicateNode#2".to_string(),
            input: "function f(): asserts a is b".to_string(),
            output: "function f(): asserts a is b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypePredicateNode#3".to_string(),
            input: "function f(): asserts this".to_string(),
            output: "function f(): asserts this;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypePredicateNode#4".to_string(),
            input: "function f(): asserts this is b".to_string(),
            output: "function f(): asserts this is b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeReferenceNode#1".to_string(),
            input: "type T = a".to_string(),
            output: "type T = a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeReferenceNode#2".to_string(),
            input: "type T = a.b".to_string(),
            output: "type T = a.b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeReferenceNode#3".to_string(),
            input: "type T = a<U>".to_string(),
            output: "type T = a<U>;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeReferenceNode#4".to_string(),
            input: "type T = a.b<U>".to_string(),
            output: "type T = a.b<U>;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "FunctionTypeNode#1".to_string(),
            input: "type T = () => a".to_string(),
            output: "type T = () => a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "FunctionTypeNode#2".to_string(),
            input: "type T = <T>() => a".to_string(),
            output: "type T = <T>() => a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "FunctionTypeNode#3".to_string(),
            input: "type T = (a) => b".to_string(),
            output: "type T = (a) => b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ConstructorTypeNode#1".to_string(),
            input: "type T = new () => a".to_string(),
            output: "type T = new () => a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ConstructorTypeNode#2".to_string(),
            input: "type T = new <T>() => a".to_string(),
            output: "type T = new <T>() => a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ConstructorTypeNode#3".to_string(),
            input: "type T = new (a) => b".to_string(),
            output: "type T = new (a) => b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ConstructorTypeNode#4".to_string(),
            input: "type T = abstract new () => a".to_string(),
            output: "type T = abstract new () => a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeQueryNode#1".to_string(),
            input: "type T = typeof a".to_string(),
            output: "type T = typeof a;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeQueryNode#2".to_string(),
            input: "type T = typeof a.b".to_string(),
            output: "type T = typeof a.b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeQueryNode#3".to_string(),
            input: "type T = typeof a<U>".to_string(),
            output: "type T = typeof a<U>;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeLiteralNode#1".to_string(),
            input: "type T = {}".to_string(),
            output: "type T = {};".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeLiteralNode#2".to_string(),
            input: "type T = {a}".to_string(),
            output: "type T = {\n    a;\n};".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrayTypeNode".to_string(),
            input: "type T = a[]".to_string(),
            output: "type T = a[];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TupleTypeNode#1".to_string(),
            input: "type T = []".to_string(),
            output: "type T = [\n];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TupleTypeNode#2".to_string(),
            input: "type T = [a]".to_string(),
            output: "type T = [\n    a\n];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TupleTypeNode#3".to_string(),
            input: "type T = [a,]".to_string(),
            output: "type T = [\n    a\n];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "RestTypeNode".to_string(),
            input: "type T = [...a]".to_string(),
            output: "type T = [\n    ...a\n];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "OptionalTypeNode".to_string(),
            input: "type T = [a?]".to_string(),
            output: "type T = [\n    a?\n];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NamedTupleMember#1".to_string(),
            input: "type T = [a: b]".to_string(),
            output: "type T = [\n    a: b\n];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NamedTupleMember#2".to_string(),
            input: "type T = [a?: b]".to_string(),
            output: "type T = [\n    a?: b\n];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "NamedTupleMember#3".to_string(),
            input: "type T = [...a: b]".to_string(),
            output: "type T = [\n    ...a: b\n];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "UnionTypeNode#1".to_string(),
            input: "type T = a | b".to_string(),
            output: "type T = a | b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "UnionTypeNode#2".to_string(),
            input: "type T = a | b | c".to_string(),
            output: "type T = a | b | c;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "UnionTypeNode#3".to_string(),
            input: "type T = | a | b".to_string(),
            output: "type T = a | b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "IntersectionTypeNode#1".to_string(),
            input: "type T = a & b".to_string(),
            output: "type T = a & b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "IntersectionTypeNode#2".to_string(),
            input: "type T = a & b & c".to_string(),
            output: "type T = a & b & c;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "IntersectionTypeNode#3".to_string(),
            input: "type T = & a & b".to_string(),
            output: "type T = a & b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ConditionalTypeNode".to_string(),
            input: "type T = a extends b ? c : d".to_string(),
            output: "type T = a extends b ? c : d;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "InferTypeNode#1".to_string(),
            input: "type T = a extends infer b ? c : d".to_string(),
            output: "type T = a extends infer b ? c : d;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "InferTypeNode#2".to_string(),
            input: "type T = a extends infer b extends c ? d : e".to_string(),
            output: "type T = a extends infer b extends c ? d : e;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ParenthesizedTypeNode".to_string(),
            input: "type T = (U)".to_string(),
            output: "type T = (U);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ThisTypeNode".to_string(),
            input: "type T = this".to_string(),
            output: "type T = this;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeOperatorNode#1".to_string(),
            input: "type T = keyof U".to_string(),
            output: "type T = keyof U;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeOperatorNode#2".to_string(),
            input: "type T = readonly U[]".to_string(),
            output: "type T = readonly U[];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeOperatorNode#3".to_string(),
            input: "type T = unique symbol".to_string(),
            output: "type T = unique symbol;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "IndexedAccessTypeNode".to_string(),
            input: "type T = a[b]".to_string(),
            output: "type T = a[b];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "IndexedAccessTypeNode with TypeQueryNode".to_string(),
            input: "type T = typeof a['b']".to_string(),
            output: "type T = (typeof a)['b'];".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MappedTypeNode#1".to_string(),
            input: "type T = { [a in b]: c }".to_string(),
            output: "type T = {\n    [a in b]: c;\n};".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MappedTypeNode#2".to_string(),
            input: "type T = { [a in b as c]: d }".to_string(),
            output: "type T = {\n    [a in b as c]: d;\n};".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MappedTypeNode#3".to_string(),
            input: "type T = { readonly [a in b]: c }".to_string(),
            output: "type T = {\n    readonly [a in b]: c;\n};".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MappedTypeNode#4".to_string(),
            input: "type T = { +readonly [a in b]: c }".to_string(),
            output: "type T = {\n    +readonly [a in b]: c;\n};".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MappedTypeNode#5".to_string(),
            input: "type T = { -readonly [a in b]: c }".to_string(),
            output: "type T = {\n    -readonly [a in b]: c;\n};".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MappedTypeNode#6".to_string(),
            input: "type T = { [a in b]?: c }".to_string(),
            output: "type T = {\n    [a in b]?: c;\n};".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MappedTypeNode#7".to_string(),
            input: "type T = { [a in b]+?: c }".to_string(),
            output: "type T = {\n    [a in b]+?: c;\n};".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MappedTypeNode#8".to_string(),
            input: "type T = { [a in b]-?: c }".to_string(),
            output: "type T = {\n    [a in b]-?: c;\n};".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MappedTypeNode#9".to_string(),
            input: "type T = { [a in b]: c; d }".to_string(),
            output: "type T = {\n    [a in b]: c;\n    d;\n};".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "LiteralTypeNode#1".to_string(),
            input: "type T = null".to_string(),
            output: "type T = null;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "LiteralTypeNode#2".to_string(),
            input: "type T = true".to_string(),
            output: "type T = true;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "LiteralTypeNode#3".to_string(),
            input: "type T = false".to_string(),
            output: "type T = false;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "LiteralTypeNode#4".to_string(),
            input: "type T = \"\"".to_string(),
            output: "type T = \"\";".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "LiteralTypeNode#5".to_string(),
            input: "type T = ''".to_string(),
            output: "type T = '';".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "LiteralTypeNode#6".to_string(),
            input: "type T = ``".to_string(),
            output: "type T = ``;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "LiteralTypeNode#7".to_string(),
            input: "type T = 0".to_string(),
            output: "type T = 0;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "LiteralTypeNode#8".to_string(),
            input: "type T = 0n".to_string(),
            output: "type T = 0n;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "LiteralTypeNode#9".to_string(),
            input: "type T = -0".to_string(),
            output: "type T = -0;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "LiteralTypeNode#10".to_string(),
            input: "type T = -0n".to_string(),
            output: "type T = -0n;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TemplateTypeNode#1".to_string(),
            input: "type T = `a${b}c`".to_string(),
            output: "type T = `a${b}c`;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TemplateTypeNode#2".to_string(),
            input: "type T = `a${b}c${d}e`".to_string(),
            output: "type T = `a${b}c${d}e`;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportTypeNode#1".to_string(),
            input: "type T = import(a)".to_string(),
            output: "type T = import(a);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportTypeNode#2".to_string(),
            input: "type T = import(a).b".to_string(),
            output: "type T = import(a).b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportTypeNode#3".to_string(),
            input: "type T = import(a).b<U>".to_string(),
            output: "type T = import(a).b<U>;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportTypeNode#4".to_string(),
            input: "type T = typeof import(a)".to_string(),
            output: "type T = typeof import(a);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportTypeNode#5".to_string(),
            input: "type T = typeof import(a).b".to_string(),
            output: "type T = typeof import(a).b;".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportTypeNode#6".to_string(),
            input: "type T = import(a, { with: { } })".to_string(),
            output: "type T = import(a, { with: {} });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportTypeNode#6".to_string(),
            input: "type T = import(a, { with: { b: \"c\" } })".to_string(),
            output: "type T = import(a, { with: { b: \"c\" } });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ImportTypeNode#7".to_string(),
            input: "type T = import(a, { with: { \"b\": \"c\" } })".to_string(),
            output: "type T = import(a, { with: { \"b\": \"c\" } });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertySignature#1".to_string(),
            input: "interface I {a}".to_string(),
            output: "interface I {\n    a;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertySignature#2".to_string(),
            input: "interface I {readonly a}".to_string(),
            output: "interface I {\n    readonly a;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertySignature#3".to_string(),
            input: "interface I {\"a\"}".to_string(),
            output: "interface I {\n    \"a\";\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertySignature#4".to_string(),
            input: "interface I {'a'}".to_string(),
            output: "interface I {\n    'a';\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertySignature#5".to_string(),
            input: "interface I {0}".to_string(),
            output: "interface I {\n    0;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertySignature#6".to_string(),
            input: "interface I {0n}".to_string(),
            output: "interface I {\n    0n;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertySignature#7".to_string(),
            input: "interface I {[a]}".to_string(),
            output: "interface I {\n    [a];\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertySignature#8".to_string(),
            input: "interface I {a?}".to_string(),
            output: "interface I {\n    a?;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertySignature#9".to_string(),
            input: "interface I {a: b}".to_string(),
            output: "interface I {\n    a: b;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodSignature#1".to_string(),
            input: "interface I {a()}".to_string(),
            output: "interface I {\n    a();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodSignature#2".to_string(),
            input: "interface I {\"a\"()}".to_string(),
            output: "interface I {\n    \"a\"();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodSignature#3".to_string(),
            input: "interface I {'a'()}".to_string(),
            output: "interface I {\n    'a'();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodSignature#4".to_string(),
            input: "interface I {0()}".to_string(),
            output: "interface I {\n    0();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodSignature#5".to_string(),
            input: "interface I {0n()}".to_string(),
            output: "interface I {\n    0n();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodSignature#6".to_string(),
            input: "interface I {[a]()}".to_string(),
            output: "interface I {\n    [a]();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodSignature#7".to_string(),
            input: "interface I {a?()}".to_string(),
            output: "interface I {\n    a?();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodSignature#8".to_string(),
            input: "interface I {a<T>()}".to_string(),
            output: "interface I {\n    a<T>();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodSignature#9".to_string(),
            input: "interface I {a(): b}".to_string(),
            output: "interface I {\n    a(): b;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodSignature#10".to_string(),
            input: "interface I {a(b): c}".to_string(),
            output: "interface I {\n    a(b): c;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "CallSignature#1".to_string(),
            input: "interface I {()}".to_string(),
            output: "interface I {\n    ();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "CallSignature#2".to_string(),
            input: "interface I {():a}".to_string(),
            output: "interface I {\n    (): a;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "CallSignature#3".to_string(),
            input: "interface I {(p)}".to_string(),
            output: "interface I {\n    (p);\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "CallSignature#4".to_string(),
            input: "interface I {<T>()}".to_string(),
            output: "interface I {\n    <T>();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ConstructSignature#1".to_string(),
            input: "interface I {new ()}".to_string(),
            output: "interface I {\n    new ();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ConstructSignature#2".to_string(),
            input: "interface I {new ():a}".to_string(),
            output: "interface I {\n    new (): a;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ConstructSignature#3".to_string(),
            input: "interface I {new (p)}".to_string(),
            output: "interface I {\n    new (p);\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ConstructSignature#4".to_string(),
            input: "interface I {new <T>()}".to_string(),
            output: "interface I {\n    new <T>();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "IndexSignatureDeclaration#1".to_string(),
            input: "interface I {[a]}".to_string(),
            output: "interface I {\n    [a];\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "IndexSignatureDeclaration#2".to_string(),
            input: "interface I {[a: b]}".to_string(),
            output: "interface I {\n    [a: b];\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "IndexSignatureDeclaration#3".to_string(),
            input: "interface I {[a: b]: c}".to_string(),
            output: "interface I {\n    [a: b]: c;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyDeclaration#1".to_string(),
            input: "class C {a}".to_string(),
            output: "class C {\n    a;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyDeclaration#2".to_string(),
            input: "class C {readonly a}".to_string(),
            output: "class C {\n    readonly a;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyDeclaration#3".to_string(),
            input: "class C {static a}".to_string(),
            output: "class C {\n    static a;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyDeclaration#4".to_string(),
            input: "class C {accessor a}".to_string(),
            output: "class C {\n    accessor a;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyDeclaration#5".to_string(),
            input: "class C {\"a\"}".to_string(),
            output: "class C {\n    \"a\";\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyDeclaration#6".to_string(),
            input: "class C {'a'}".to_string(),
            output: "class C {\n    'a';\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyDeclaration#7".to_string(),
            input: "class C {0}".to_string(),
            output: "class C {\n    0;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyDeclaration#8".to_string(),
            input: "class C {0n}".to_string(),
            output: "class C {\n    0n;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyDeclaration#9".to_string(),
            input: "class C {[a]}".to_string(),
            output: "class C {\n    [a];\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyDeclaration#10".to_string(),
            input: "class C {#a}".to_string(),
            output: "class C {\n    #a;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyDeclaration#11".to_string(),
            input: "class C {a?}".to_string(),
            output: "class C {\n    a?;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyDeclaration#12".to_string(),
            input: "class C {a!}".to_string(),
            output: "class C {\n    a!;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyDeclaration#13".to_string(),
            input: "class C {a: b}".to_string(),
            output: "class C {\n    a: b;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyDeclaration#14".to_string(),
            input: "class C {a = b}".to_string(),
            output: "class C {\n    a = b;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "PropertyDeclaration#15".to_string(),
            input: "class C {@a b}".to_string(),
            output: "class C {\n    @a\n    b;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodDeclaration#1".to_string(),
            input: "class C {a()}".to_string(),
            output: "class C {\n    a();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodDeclaration#2".to_string(),
            input: "class C {\"a\"()}".to_string(),
            output: "class C {\n    \"a\"();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodDeclaration#3".to_string(),
            input: "class C {'a'()}".to_string(),
            output: "class C {\n    'a'();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodDeclaration#4".to_string(),
            input: "class C {0()}".to_string(),
            output: "class C {\n    0();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodDeclaration#5".to_string(),
            input: "class C {0n()}".to_string(),
            output: "class C {\n    0n();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodDeclaration#6".to_string(),
            input: "class C {[a]()}".to_string(),
            output: "class C {\n    [a]();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodDeclaration#7".to_string(),
            input: "class C {#a()}".to_string(),
            output: "class C {\n    #a();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodDeclaration#8".to_string(),
            input: "class C {a?()}".to_string(),
            output: "class C {\n    a?();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodDeclaration#9".to_string(),
            input: "class C {a<T>()}".to_string(),
            output: "class C {\n    a<T>();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodDeclaration#10".to_string(),
            input: "class C {a(): b}".to_string(),
            output: "class C {\n    a(): b;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodDeclaration#11".to_string(),
            input: "class C {a(b): c}".to_string(),
            output: "class C {\n    a(b): c;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodDeclaration#12".to_string(),
            input: "class C {a() {} }".to_string(),
            output: "class C {\n    a() { }\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodDeclaration#13".to_string(),
            input: "class C {@a b() {} }".to_string(),
            output: "class C {\n    @a\n    b() { }\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodDeclaration#14".to_string(),
            input: "class C {static a() {} }".to_string(),
            output: "class C {\n    static a() { }\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "MethodDeclaration#15".to_string(),
            input: "class C {async a() {} }".to_string(),
            output: "class C {\n    async a() { }\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "GetAccessorDeclaration#1".to_string(),
            input: "class C {get a()}".to_string(),
            output: "class C {\n    get a();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "GetAccessorDeclaration#2".to_string(),
            input: "class C {get \"a\"()}".to_string(),
            output: "class C {\n    get \"a\"();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "GetAccessorDeclaration#3".to_string(),
            input: "class C {get 'a'()}".to_string(),
            output: "class C {\n    get 'a'();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "GetAccessorDeclaration#4".to_string(),
            input: "class C {get 0()}".to_string(),
            output: "class C {\n    get 0();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "GetAccessorDeclaration#5".to_string(),
            input: "class C {get 0n()}".to_string(),
            output: "class C {\n    get 0n();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "GetAccessorDeclaration#6".to_string(),
            input: "class C {get [a]()}".to_string(),
            output: "class C {\n    get [a]();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "GetAccessorDeclaration#7".to_string(),
            input: "class C {get #a()}".to_string(),
            output: "class C {\n    get #a();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "GetAccessorDeclaration#8".to_string(),
            input: "class C {get a(): b}".to_string(),
            output: "class C {\n    get a(): b;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "GetAccessorDeclaration#9".to_string(),
            input: "class C {get a(b): c}".to_string(),
            output: "class C {\n    get a(b): c;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "GetAccessorDeclaration#10".to_string(),
            input: "class C {get a() {} }".to_string(),
            output: "class C {\n    get a() { }\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "GetAccessorDeclaration#11".to_string(),
            input: "class C {@a get b() {} }".to_string(),
            output: "class C {\n    @a\n    get b() { }\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "GetAccessorDeclaration#12".to_string(),
            input: "class C {static get a() {} }".to_string(),
            output: "class C {\n    static get a() { }\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "SetAccessorDeclaration#1".to_string(),
            input: "class C {set a()}".to_string(),
            output: "class C {\n    set a();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "SetAccessorDeclaration#2".to_string(),
            input: "class C {set \"a\"()}".to_string(),
            output: "class C {\n    set \"a\"();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "SetAccessorDeclaration#3".to_string(),
            input: "class C {set 'a'()}".to_string(),
            output: "class C {\n    set 'a'();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "SetAccessorDeclaration#4".to_string(),
            input: "class C {set 0()}".to_string(),
            output: "class C {\n    set 0();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "SetAccessorDeclaration#5".to_string(),
            input: "class C {set 0n()}".to_string(),
            output: "class C {\n    set 0n();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "SetAccessorDeclaration#6".to_string(),
            input: "class C {set [a]()}".to_string(),
            output: "class C {\n    set [a]();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "SetAccessorDeclaration#7".to_string(),
            input: "class C {set #a()}".to_string(),
            output: "class C {\n    set #a();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "SetAccessorDeclaration#8".to_string(),
            input: "class C {set a(): b}".to_string(),
            output: "class C {\n    set a(): b;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "SetAccessorDeclaration#9".to_string(),
            input: "class C {set a(b): c}".to_string(),
            output: "class C {\n    set a(b): c;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "SetAccessorDeclaration#10".to_string(),
            input: "class C {set a() {} }".to_string(),
            output: "class C {\n    set a() { }\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "SetAccessorDeclaration#11".to_string(),
            input: "class C {@a set b() {} }".to_string(),
            output: "class C {\n    @a\n    set b() { }\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "SetAccessorDeclaration#12".to_string(),
            input: "class C {static set a() {} }".to_string(),
            output: "class C {\n    static set a() { }\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ConstructorDeclaration#1".to_string(),
            input: "class C {constructor()}".to_string(),
            output: "class C {\n    constructor();\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ConstructorDeclaration#2".to_string(),
            input: "class C {constructor(): b}".to_string(),
            output: "class C {\n    constructor(): b;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ConstructorDeclaration#3".to_string(),
            input: "class C {constructor(b): c}".to_string(),
            output: "class C {\n    constructor(b): c;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ConstructorDeclaration#4".to_string(),
            input: "class C {constructor() {} }".to_string(),
            output: "class C {\n    constructor() { }\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ConstructorDeclaration#5".to_string(),
            input: "class C {@a constructor() {} }".to_string(),
            output: "class C {\n    constructor() { }\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ConstructorDeclaration#6".to_string(),
            input: "class C {private constructor() {} }".to_string(),
            output: "class C {\n    private constructor() { }\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ClassStaticBlockDeclaration".to_string(),
            input: "class C {static { }}".to_string(),
            output: "class C {\n    static { }\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "SemicolonClassElement#1".to_string(),
            input: "class C {;}".to_string(),
            output: "class C {\n    ;\n}".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ParameterDeclaration#1".to_string(),
            input: "function f(a)".to_string(),
            output: "function f(a);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ParameterDeclaration#2".to_string(),
            input: "function f(a: b)".to_string(),
            output: "function f(a: b);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ParameterDeclaration#3".to_string(),
            input: "function f(a = b)".to_string(),
            output: "function f(a = b);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ParameterDeclaration#4".to_string(),
            input: "function f(a?)".to_string(),
            output: "function f(a?);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ParameterDeclaration#5".to_string(),
            input: "function f(...a)".to_string(),
            output: "function f(...a);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ParameterDeclaration#6".to_string(),
            input: "function f(this)".to_string(),
            output: "function f(this);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ObjectBindingPattern#1".to_string(),
            input: "function f({})".to_string(),
            output: "function f({});".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ObjectBindingPattern#2".to_string(),
            input: "function f({a})".to_string(),
            output: "function f({ a });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ObjectBindingPattern#3".to_string(),
            input: "function f({a = b})".to_string(),
            output: "function f({ a = b });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ObjectBindingPattern#4".to_string(),
            input: "function f({a: b})".to_string(),
            output: "function f({ a: b });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ObjectBindingPattern#5".to_string(),
            input: "function f({a: b = c})".to_string(),
            output: "function f({ a: b = c });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ObjectBindingPattern#6".to_string(),
            input: "function f({\"a\": b})".to_string(),
            output: "function f({ \"a\": b });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ObjectBindingPattern#7".to_string(),
            input: "function f({'a': b})".to_string(),
            output: "function f({ 'a': b });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ObjectBindingPattern#8".to_string(),
            input: "function f({0: b})".to_string(),
            output: "function f({ 0: b });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ObjectBindingPattern#9".to_string(),
            input: "function f({[a]: b})".to_string(),
            output: "function f({ [a]: b });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ObjectBindingPattern#10".to_string(),
            input: "function f({...a})".to_string(),
            output: "function f({ ...a });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ObjectBindingPattern#11".to_string(),
            input: "function f({a: {}})".to_string(),
            output: "function f({ a: {} });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ObjectBindingPattern#12".to_string(),
            input: "function f({a: []})".to_string(),
            output: "function f({ a: [] });".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrayBindingPattern#1".to_string(),
            input: "function f([])".to_string(),
            output: "function f([]);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrayBindingPattern#2".to_string(),
            input: "function f([,])".to_string(),
            output: "function f([,]);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrayBindingPattern#3".to_string(),
            input: "function f([a])".to_string(),
            output: "function f([a]);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrayBindingPattern#4".to_string(),
            input: "function f([a, b])".to_string(),
            output: "function f([a, b]);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrayBindingPattern#5".to_string(),
            input: "function f([a, , b])".to_string(),
            output: "function f([a, , b]);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrayBindingPattern#6".to_string(),
            input: "function f([a = b])".to_string(),
            output: "function f([a = b]);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrayBindingPattern#7".to_string(),
            input: "function f([...a])".to_string(),
            output: "function f([...a]);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrayBindingPattern#8".to_string(),
            input: "function f([{}])".to_string(),
            output: "function f([{}]);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "ArrayBindingPattern#9".to_string(),
            input: "function f([[]])".to_string(),
            output: "function f([[]]);".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeParameterDeclaration#1".to_string(),
            input: "function f<T>();".to_string(),
            output: "function f<T>();".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeParameterDeclaration#2".to_string(),
            input: "function f<in T>();".to_string(),
            output: "function f<in T>();".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeParameterDeclaration#3".to_string(),
            input: "function f<T extends U>();".to_string(),
            output: "function f<T extends U>();".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeParameterDeclaration#4".to_string(),
            input: "function f<T = U>();".to_string(),
            output: "function f<T = U>();".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeParameterDeclaration#5".to_string(),
            input: "function f<T extends U = V>();".to_string(),
            output: "function f<T extends U = V>();".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "TypeParameterDeclaration#6".to_string(),
            input: "function f<T, U>();".to_string(),
            output: "function f<T, U>();".to_string(),
            jsx: false,
        },
        EmitCase {
            title: "JsxElement1".to_string(),
            input: "<a></a>".to_string(),
            output: "<a></a>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxElement2".to_string(),
            input: "<this></this>".to_string(),
            output: "<this></this>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxElement3".to_string(),
            input: "<a:b></a:b>".to_string(),
            output: "<a:b></a:b>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxElement4".to_string(),
            input: "<a.b></a.b>".to_string(),
            output: "<a.b></a.b>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxElement5".to_string(),
            input: "<a<b>></a>".to_string(),
            output: "<a<b>></a>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxElement6".to_string(),
            input: "<a b></a>".to_string(),
            output: "<a b></a>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxElement7".to_string(),
            input: "<a>b</a>".to_string(),
            output: "<a>b</a>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxElement8".to_string(),
            input: "<a>{b}</a>".to_string(),
            output: "<a>{b}</a>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxElement9".to_string(),
            input: "<a><b></b></a>".to_string(),
            output: "<a><b></b></a>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxElement10".to_string(),
            input: "<a><b /></a>".to_string(),
            output: "<a><b /></a>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxElement11".to_string(),
            input: "<a><></></a>".to_string(),
            output: "<a><></></a>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxElement12".to_string(),
            input: "<a>\n    {/* missing */}\n    {\n        // foo\n    }\n</a>".to_string(),
            output: "<a>\n    {/* missing */}\n    {\n    // foo\n    }\n</a>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxSelfClosingElement1".to_string(),
            input: "<a />".to_string(),
            output: "<a />;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxSelfClosingElement2".to_string(),
            input: "<this />".to_string(),
            output: "<this />;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxSelfClosingElement3".to_string(),
            input: "<a:b />".to_string(),
            output: "<a:b />;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxSelfClosingElement4".to_string(),
            input: "<a.b />".to_string(),
            output: "<a.b />;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxSelfClosingElement5".to_string(),
            input: "<a<b> />".to_string(),
            output: "<a<b> />;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxSelfClosingElement6".to_string(),
            input: "<a b/>".to_string(),
            output: "<a b/>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxFragment1".to_string(),
            input: "<></>".to_string(),
            output: "<></>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxFragment2".to_string(),
            input: "<>b</>".to_string(),
            output: "<>b</>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxFragment3".to_string(),
            input: "<>{b}</>".to_string(),
            output: "<>{b}</>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxFragment4".to_string(),
            input: "<><b></b></>".to_string(),
            output: "<><b></b></>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxFragment5".to_string(),
            input: "<><b /></>".to_string(),
            output: "<><b /></>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxFragment6".to_string(),
            input: "<><></></>".to_string(),
            output: "<><></></>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxAttribute1".to_string(),
            input: "<a b/>".to_string(),
            output: "<a b/>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxAttribute2".to_string(),
            input: "<a b:c/>".to_string(),
            output: "<a b:c/>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxAttribute3".to_string(),
            input: "<a b=\"c\"/>".to_string(),
            output: "<a b=\"c\"/>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxAttribute4".to_string(),
            input: "<a b='c'/>".to_string(),
            output: "<a b='c'/>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxAttribute5".to_string(),
            input: "<a b={c}/>".to_string(),
            output: "<a b={c}/>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxAttribute6".to_string(),
            input: "<a b=<c></c>/>".to_string(),
            output: "<a b=<c></c>/>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxAttribute7".to_string(),
            input: "<a b=<c />/>".to_string(),
            output: "<a b=<c />/>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxAttribute8".to_string(),
            input: "<a b=<></>/>".to_string(),
            output: "<a b=<></>/>;".to_string(),
            jsx: true,
        },
        EmitCase {
            title: "JsxSpreadAttribute".to_string(),
            input: "<a {...b}/>".to_string(),
            output: "<a {...b}/>;".to_string(),
            jsx: true,
        },
    ]
}

fn parse_typescript(input: &str, jsx: bool) -> ast::SourceFile {
    parser::parse_source_file(
        ast::SourceFileParseOptions {
            file_name: if jsx { "/file.tsx" } else { "/file.ts" }.to_string(),
            path: if jsx { "/file.tsx" } else { "/file.ts" }.to_string(),
            ..Default::default()
        },
        input.to_string(),
        if jsx {
            core::ScriptKind::TSX
        } else {
            core::ScriptKind::TS
        },
    )
}

trait EmitInput {
    fn emit_with(self, printer: &mut Printer) -> String;
}

impl EmitInput for ast::SourceFile {
    fn emit_with(self, printer: &mut Printer) -> String {
        printer.emit(&self.as_node(), Some(&self))
    }
}

impl EmitInput for ast::Node {
    fn emit_with(self, printer: &mut Printer) -> String {
        printer.emit(&self, None)
    }
}

fn check_emit<T: EmitInput>(file: T, expected: &str) {
    let mut printer = new_printer(PrinterOptions::default(), PrintHandlers::default(), None);
    let actual = file.emit_with(&mut printer);
    assert_eq!(expected, trim_emit_final_newline(&actual));
}

fn check_emit_with_context<T: EmitInput>(file: T, context: crate::EmitContext, expected: &str) {
    let mut printer = new_printer(
        PrinterOptions::default(),
        PrintHandlers::default(),
        Some(context),
    );
    let actual = file.emit_with(&mut printer);
    assert_eq!(expected, trim_emit_final_newline(&actual));
}

fn trim_emit_final_newline(text: &str) -> &str {
    text.strip_suffix('\n').unwrap_or(text)
}

#[test]
fn test_emit_mapped_type_keyof_constraint() {
    check_emit(
        parse_typescript("type T<U> = { -readonly [P in keyof U]: U[P]; };", false),
        "type T<U> = {\n    -readonly [P in keyof U]: U[P];\n};",
    );
}

fn node_list(
    factory: &mut ast::NodeFactory,
    nodes: impl IntoIterator<Item = ast::Node>,
) -> ast::NodeList {
    factory.new_node_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        nodes,
    )
}

fn modifier_list(
    factory: &mut ast::NodeFactory,
    modifiers: impl IntoIterator<Item = ast::Node>,
) -> ast::ModifierList {
    factory.new_modifier_list(
        core::undefined_text_range(),
        core::undefined_text_range(),
        modifiers,
        ast::ModifierFlags::NONE,
    )
}

fn source_file_from_statements(
    mut factory: ast::NodeFactory,
    statements: impl IntoIterator<Item = ast::Node>,
) -> ast::SourceFile {
    let statements = node_list(&mut factory, statements);
    let eof = factory.new_token(ast::Kind::EndOfFile);
    let root = factory.new_source_file(
        ast::SourceFileParseOptions {
            file_name: "/file.ts".to_string(),
            path: "/file.ts".to_string(),
            ..Default::default()
        },
        "",
        statements,
        eof,
    );
    factory.finish_parsed_source_file(root, ast::ParsedSourceFileMetadata::default())
}

fn binary(
    factory: &mut ast::NodeFactory,
    left_text: &str,
    operator_kind: ast::Kind,
    right_text: &str,
) -> ast::Node {
    let left = factory.new_identifier(left_text);
    let operator = factory.new_token(operator_kind);
    let right = factory.new_identifier(right_text);
    factory.new_binary_expression(None, left, None, operator, right)
}

fn source_file_from_expression(
    factory: ast::NodeFactory,
    expression: ast::Node,
) -> ast::SourceFile {
    let mut factory = factory;
    let statement = factory.new_expression_statement(expression);
    source_file_from_statements(factory, [statement])
}

fn type_ref(factory: &mut ast::NodeFactory, name: &str) -> ast::Node {
    let name = factory.new_identifier(name);
    factory.new_type_reference_node(name, None)
}

fn empty_arrow_function(factory: &mut ast::NodeFactory) -> ast::Node {
    let parameters = node_list(factory, []);
    let equals = factory.new_token(ast::Kind::EqualsGreaterThanToken);
    let statements = node_list(factory, []);
    let body = factory.new_block(statements, false);
    factory.new_arrow_function(None, None, parameters, None, None, equals, body)
}

fn empty_block(factory: &mut ast::NodeFactory) -> ast::Node {
    let statements = node_list(factory, []);
    factory.new_block(statements, false)
}

fn empty_function_expression(factory: &mut ast::NodeFactory) -> ast::Node {
    let parameters = node_list(factory, []);
    let body = empty_block(factory);
    factory.new_function_expression(None, None, None, None, parameters, None, None, body)
}

fn empty_class_expression(factory: &mut ast::NodeFactory) -> ast::Node {
    let members = node_list(factory, []);
    factory.new_class_expression(None, None, None, None, members)
}

fn type_alias_file(mut factory: ast::NodeFactory, type_node: ast::Node) -> ast::SourceFile {
    let name = factory.new_identifier("_");
    let declaration = factory.new_type_alias_declaration(None, name, None, type_node);
    source_file_from_statements(factory, [declaration])
}

fn named_union_type(factory: &mut ast::NodeFactory, names: &[&str]) -> ast::Node {
    let types = names
        .iter()
        .map(|name| type_ref(factory, name))
        .collect::<Vec<_>>();
    let types = node_list(factory, types);
    factory.new_union_type_node(types)
}

fn function_type(factory: &mut ast::NodeFactory, return_type: ast::Node) -> ast::Node {
    let parameters = node_list(factory, []);
    factory.new_function_type_node(None, parameters, return_type)
}

fn constrained_infer_type(
    factory: &mut ast::NodeFactory,
    name: &str,
    constraint: &str,
) -> ast::Node {
    let name = factory.new_identifier(name);
    let constraint = type_ref(factory, constraint);
    let type_parameter = factory.new_type_parameter_declaration(None, name, constraint, None, None);
    factory.new_infer_type_node(type_parameter)
}

fn binary_side(factory: &mut ast::NodeFactory, label: &str, kind: Option<ast::Kind>) -> ast::Node {
    match kind {
        None | Some(ast::Kind::Unknown) | Some(ast::Kind::Identifier) => {
            factory.new_identifier(label)
        }
        Some(ast::Kind::ArrowFunction) => empty_arrow_function(factory),
        Some(kind) if ast::is_binary_operator(kind) => {
            binary(factory, &format!("{label}l"), kind, &format!("{label}r"))
        }
        Some(kind) => panic!("unsupported test side kind: {kind:?}"),
    }
}

#[test]
fn embedded_empty_statement_uses_punctuation_semicolon() {
    let file = parse_typescript("if (a)\n;", false);
    assert!(file.diagnostics().is_empty());

    let semicolon_writes = Rc::new(RefCell::new(Vec::new()));
    let writer = share_text_writer(Box::new(SemicolonRecordingWriter {
        inner: new_text_writer("\n".to_string(), 4),
        semicolon_writes: semicolon_writes.clone(),
    }));
    let root = file.as_node();
    let mut printer = new_printer(PrinterOptions::default(), PrintHandlers::default(), None);

    printer.write_node(Some(&root), Some(&file), writer.clone(), None);

    assert_eq!("if (a)\n    ;\n", writer.borrow().string());
    assert_eq!(
        *semicolon_writes.borrow(),
        vec![SemicolonWrite::Punctuation]
    );
}

#[test]
fn function_body_emit_uses_notification_hooks_and_no_source_map_flag() {
    let file = parse_typescript("function f() {}", false);
    assert!(file.diagnostics().is_empty());
    let function = file
        .statements_view()
        .iter()
        .next()
        .expect("source should have a function declaration");
    let body = file
        .store()
        .body(function)
        .expect("function should have a body");
    let events = Rc::new(RefCell::new(Vec::new()));
    let before_events = events.clone();
    let after_events = events.clone();
    let before_body = body;
    let after_body = body;
    let handlers = PrintHandlers {
        on_before_emit_node: Some(Box::new(move |node| {
            if node.is_some_and(|node| *node == before_body) {
                before_events.borrow_mut().push("before-body");
            }
        })),
        on_after_emit_node: Some(Box::new(move |node| {
            if node.is_some_and(|node| *node == after_body) {
                after_events.borrow_mut().push("after-body");
            }
        })),
        ..PrintHandlers::default()
    };
    let mut printer = new_printer(PrinterOptions::default(), handlers, None);

    let _ = file.emit_with(&mut printer);

    assert_eq!(*events.borrow(), vec!["before-body", "after-body"]);
    let mut emit_context = printer.into_emit_context();
    assert_eq!(
        emit_context.emit_flags(&body) & crate::EF_NO_SOURCE_MAP,
        crate::EF_NO_SOURCE_MAP
    );
}

#[test]
fn function_body_close_brace_uses_token_source_map() {
    let file = parse_typescript("function f() {\n}", false);
    assert!(file.diagnostics().is_empty());
    let root = file.as_node();
    let writer = crate::new_shared_text_writer("\n".to_string(), 4);
    let source_map_generator = sourcemap::new_generator(
        "file.js".to_string(),
        String::new(),
        "/".to_string(),
        tspath::ComparePathsOptions {
            use_case_sensitive_file_names: true,
            current_directory: "/".to_string(),
        },
    );
    let mut printer = new_printer(PrinterOptions::default(), PrintHandlers::default(), None);

    let mut source_map_generator = printer
        .write_node(Some(&root), Some(&file), writer, Some(source_map_generator))
        .expect("source map generator should be returned");

    let raw_source_map = source_map_generator.raw_source_map();
    let mappings = sourcemap::decode_mappings(raw_source_map.mappings).collect::<Vec<_>>();
    assert!(
        mappings.iter().any(|mapping| mapping.is_source_mapping()
            && mapping.source_line == 1
            && mapping.source_character == 0),
        "expected a source-map segment for the function body close brace"
    );
}

#[test]
fn test_emit() {
    let data = emit_cases_from_go_source();

    for rec in data.iter() {
        let file = parse_typescript(&rec.input, rec.jsx);
        assert!(
            file.diagnostics().is_empty(),
            "{}: unexpected parse diagnostics",
            rec.title
        );
        let mut printer = new_printer(PrinterOptions::default(), PrintHandlers::default(), None);
        let actual = file.emit_with(&mut printer);
        assert_eq!(
            rec.output,
            trim_emit_final_newline(&actual),
            "{}",
            rec.title
        );
    }
}

#[test]
fn test_parenthesize_decorator() {
    let mut factory = ast::NodeFactory::default();
    let expression = binary(&mut factory, "a", ast::Kind::PlusToken, "b");
    let decorator = factory.new_decorator(expression);
    let modifiers = modifier_list(&mut factory, [decorator]);
    let name = factory.new_identifier("C");
    let members = node_list(&mut factory, []);
    let class = factory.new_class_declaration(modifiers, name, None, None, members);
    let file = source_file_from_statements(factory, [class]);

    check_emit(file, "@(a + b)\nclass C {\n}");
}

#[test]
fn test_parenthesize_computed_property_name() {
    let mut factory = ast::NodeFactory::default();
    let comma = binary(&mut factory, "a", ast::Kind::CommaToken, "b");
    let name = factory.new_computed_property_name(comma);
    let property = factory.new_property_declaration(None, name, None, None, None);
    let members = node_list(&mut factory, [property]);
    let class_name = factory.new_identifier("C");
    let class = factory.new_class_declaration(None, class_name, None, None, members);
    let file = source_file_from_statements(factory, [class]);

    check_emit(file, "class C {\n    [(a, b)];\n}");
}

#[test]
fn test_parenthesize_array_literal() {
    let mut factory = ast::NodeFactory::default();
    let comma = binary(&mut factory, "a", ast::Kind::CommaToken, "b");
    let elements = node_list(&mut factory, [comma]);
    let array = factory.new_array_literal_expression(elements, false);
    let statement = factory.new_expression_statement(array);
    let file = source_file_from_statements(factory, [statement]);

    check_emit(file, "[(a, b)];");
}

#[test]
fn test_parenthesize_property_access() {
    let cases: [(&str, fn(&mut ast::NodeFactory) -> ast::Node); 3] = [
        ("(a, b).c;", |factory| {
            let expression = binary(factory, "a", ast::Kind::CommaToken, "b");
            let name = factory.new_identifier("c");
            factory.new_property_access_expression(expression, None, name, ast::NodeFlags::NONE)
        }),
        ("(a?.b).c;", |factory| {
            let receiver = factory.new_identifier("a");
            let question_dot = factory.new_token(ast::Kind::QuestionDotToken);
            let optional_name = factory.new_identifier("b");
            let optional = factory.new_property_access_expression(
                receiver,
                question_dot,
                optional_name,
                ast::NodeFlags::OPTIONAL_CHAIN,
            );
            let name = factory.new_identifier("c");
            factory.new_property_access_expression(optional, None, name, ast::NodeFlags::NONE)
        }),
        ("(new a).b;", |factory| {
            let expression = factory.new_identifier("a");
            let new_expression = factory.new_new_expression(expression, None, None);
            let name = factory.new_identifier("b");
            factory.new_property_access_expression(new_expression, None, name, ast::NodeFlags::NONE)
        }),
    ];

    for (expected, build) in cases {
        let mut factory = ast::NodeFactory::default();
        let expression = build(&mut factory);
        let statement = factory.new_expression_statement(expression);
        let file = source_file_from_statements(factory, [statement]);
        check_emit(file, expected);
    }
}

#[test]
fn test_parenthesize_element_access() {
    let cases: [(&str, fn(&mut ast::NodeFactory) -> ast::Node); 3] = [
        ("(a, b)[c];", |factory| {
            let expression = binary(factory, "a", ast::Kind::CommaToken, "b");
            let argument = factory.new_identifier("c");
            factory.new_element_access_expression(expression, None, argument, ast::NodeFlags::NONE)
        }),
        ("(a?.b)[c];", |factory| {
            let receiver = factory.new_identifier("a");
            let question_dot = factory.new_token(ast::Kind::QuestionDotToken);
            let optional_name = factory.new_identifier("b");
            let optional = factory.new_property_access_expression(
                receiver,
                question_dot,
                optional_name,
                ast::NodeFlags::OPTIONAL_CHAIN,
            );
            let argument = factory.new_identifier("c");
            factory.new_element_access_expression(optional, None, argument, ast::NodeFlags::NONE)
        }),
        ("(new a)[b];", |factory| {
            let expression = factory.new_identifier("a");
            let new_expression = factory.new_new_expression(expression, None, None);
            let argument = factory.new_identifier("b");
            factory.new_element_access_expression(
                new_expression,
                None,
                argument,
                ast::NodeFlags::NONE,
            )
        }),
    ];

    for (expected, build) in cases {
        let mut factory = ast::NodeFactory::default();
        let expression = build(&mut factory);
        let statement = factory.new_expression_statement(expression);
        let file = source_file_from_statements(factory, [statement]);
        check_emit(file, expected);
    }
}

#[test]
fn test_parenthesize_call_expression() {
    let cases: [(&str, fn(&mut ast::NodeFactory) -> ast::Node); 4] = [
        ("(a, b)();", |factory| {
            let callee = binary(factory, "a", ast::Kind::CommaToken, "b");
            let arguments = node_list(factory, []);
            factory.new_call_expression(callee, None, None, arguments, ast::NodeFlags::NONE)
        }),
        ("(a?.b)();", |factory| {
            let receiver = factory.new_identifier("a");
            let question_dot = factory.new_token(ast::Kind::QuestionDotToken);
            let name = factory.new_identifier("b");
            let callee = factory.new_property_access_expression(
                receiver,
                question_dot,
                name,
                ast::NodeFlags::OPTIONAL_CHAIN,
            );
            let arguments = node_list(factory, []);
            factory.new_call_expression(callee, None, None, arguments, ast::NodeFlags::NONE)
        }),
        ("(new C)();", |factory| {
            let class_name = factory.new_identifier("C");
            let callee = factory.new_new_expression(class_name, None, None);
            let arguments = node_list(factory, []);
            factory.new_call_expression(callee, None, None, arguments, ast::NodeFlags::NONE)
        }),
        ("a((b, c));", |factory| {
            let callee = factory.new_identifier("a");
            let argument = binary(factory, "b", ast::Kind::CommaToken, "c");
            let arguments = node_list(factory, [argument]);
            factory.new_call_expression(callee, None, None, arguments, ast::NodeFlags::NONE)
        }),
    ];

    for (expected, build) in cases {
        let mut factory = ast::NodeFactory::default();
        let expression = build(&mut factory);
        let statement = factory.new_expression_statement(expression);
        let file = source_file_from_statements(factory, [statement]);
        check_emit(file, expected);
    }
}

#[test]
fn test_parenthesize_new_expression() {
    let cases: [(&str, fn(&mut ast::NodeFactory) -> ast::Node); 3] = [
        ("new (a, b)();", |factory| {
            let expression = binary(factory, "a", ast::Kind::CommaToken, "b");
            let arguments = node_list(factory, []);
            factory.new_new_expression(expression, None, arguments)
        }),
        ("new (C());", |factory| {
            let expression = factory.new_identifier("C");
            let arguments = node_list(factory, []);
            let call = factory.new_call_expression(
                expression,
                None,
                None,
                arguments,
                ast::NodeFlags::NONE,
            );
            factory.new_new_expression(call, None, None)
        }),
        ("new C((a, b));", |factory| {
            let expression = factory.new_identifier("C");
            let argument = binary(factory, "a", ast::Kind::CommaToken, "b");
            let arguments = node_list(factory, [argument]);
            factory.new_new_expression(expression, None, arguments)
        }),
    ];

    for (expected, build) in cases {
        let mut factory = ast::NodeFactory::default();
        let expression = build(&mut factory);
        let statement = factory.new_expression_statement(expression);
        let file = source_file_from_statements(factory, [statement]);
        check_emit(file, expected);
    }
}

#[test]
fn test_parenthesize_tagged_template() {
    let cases: [(&str, fn(&mut ast::NodeFactory) -> ast::Node); 2] = [
        ("(a, b) ``;", |factory| {
            let tag = binary(factory, "a", ast::Kind::CommaToken, "b");
            let template = factory.new_no_substitution_template_literal("", ast::TokenFlags::NONE);
            factory.new_tagged_template_expression(tag, None, None, template, ast::NodeFlags::NONE)
        }),
        ("(a?.b) ``;", |factory| {
            let receiver = factory.new_identifier("a");
            let question_dot = factory.new_token(ast::Kind::QuestionDotToken);
            let name = factory.new_identifier("b");
            let tag = factory.new_property_access_expression(
                receiver,
                question_dot,
                name,
                ast::NodeFlags::OPTIONAL_CHAIN,
            );
            let template = factory.new_no_substitution_template_literal("", ast::TokenFlags::NONE);
            factory.new_tagged_template_expression(tag, None, None, template, ast::NodeFlags::NONE)
        }),
    ];

    for (expected, build) in cases {
        let mut factory = ast::NodeFactory::default();
        let expression = build(&mut factory);
        let statement = factory.new_expression_statement(expression);
        let file = source_file_from_statements(factory, [statement]);
        check_emit(file, expected);
    }
}

#[test]
fn test_parenthesize_type_assertion_and_unary_like_expressions() {
    let cases: [(&str, fn(&mut ast::NodeFactory) -> ast::Node); 5] = [
        ("<T>(a + b);", |factory| {
            let type_name = factory.new_identifier("T");
            let type_node = factory.new_type_reference_node(type_name, None);
            let expression = binary(factory, "a", ast::Kind::PlusToken, "b");
            factory.new_type_assertion(type_node, expression)
        }),
        ("delete (a + b);", |factory| {
            let expression = binary(factory, "a", ast::Kind::PlusToken, "b");
            factory.new_delete_expression(expression)
        }),
        ("void (a + b);", |factory| {
            let expression = binary(factory, "a", ast::Kind::PlusToken, "b");
            factory.new_void_expression(expression)
        }),
        ("typeof (a + b);", |factory| {
            let expression = binary(factory, "a", ast::Kind::PlusToken, "b");
            factory.new_type_of_expression(expression)
        }),
        ("await (a + b);", |factory| {
            let expression = binary(factory, "a", ast::Kind::PlusToken, "b");
            factory.new_await_expression(expression)
        }),
    ];

    for (expected, build) in cases {
        let mut factory = ast::NodeFactory::default();
        let expression = build(&mut factory);
        let statement = factory.new_expression_statement(expression);
        let file = source_file_from_statements(factory, [statement]);
        check_emit(file, expected);
    }
}

#[test]
fn test_parenthesize_arrow_function_body() {
    let cases: [(&str, fn(&mut ast::NodeFactory) -> ast::Node); 2] = [
        ("() => ({});", |factory| {
            let parameters = node_list(factory, []);
            let equals = factory.new_token(ast::Kind::EqualsGreaterThanToken);
            let properties = node_list(factory, []);
            let body = factory.new_object_literal_expression(properties, false);
            factory.new_arrow_function(None, None, parameters, None, None, equals, body)
        }),
        ("() => ({}.a);", |factory| {
            let parameters = node_list(factory, []);
            let equals = factory.new_token(ast::Kind::EqualsGreaterThanToken);
            let properties = node_list(factory, []);
            let object = factory.new_object_literal_expression(properties, false);
            let name = factory.new_identifier("a");
            let body =
                factory.new_property_access_expression(object, None, name, ast::NodeFlags::NONE);
            factory.new_arrow_function(None, None, parameters, None, None, equals, body)
        }),
    ];

    for (expected, build) in cases {
        let mut factory = ast::NodeFactory::default();
        let expression = build(&mut factory);
        let statement = factory.new_expression_statement(expression);
        let file = source_file_from_statements(factory, [statement]);
        check_emit(file, expected);
    }
}

#[test]
fn test_parenthesize_binary() {
    let cases = [
        (None, ast::Kind::CommaToken, None, "l, r"),
        (
            Some(ast::Kind::PlusToken),
            ast::Kind::CommaToken,
            None,
            "ll + lr, r",
        ),
        (
            Some(ast::Kind::PlusToken),
            ast::Kind::AsteriskToken,
            None,
            "(ll + lr) * r",
        ),
        (
            None,
            ast::Kind::AsteriskToken,
            Some(ast::Kind::PlusToken),
            "l * (rl + rr)",
        ),
        (
            Some(ast::Kind::AsteriskToken),
            ast::Kind::PlusToken,
            None,
            "ll * lr + r",
        ),
        (
            None,
            ast::Kind::PlusToken,
            Some(ast::Kind::AsteriskToken),
            "l + rl * rr",
        ),
        (
            Some(ast::Kind::AsteriskToken),
            ast::Kind::SlashToken,
            None,
            "ll * lr / r",
        ),
        (
            Some(ast::Kind::AsteriskAsteriskToken),
            ast::Kind::SlashToken,
            None,
            "ll ** lr / r",
        ),
        (
            Some(ast::Kind::AsteriskToken),
            ast::Kind::AsteriskAsteriskToken,
            None,
            "(ll * lr) ** r",
        ),
        (
            Some(ast::Kind::AsteriskAsteriskToken),
            ast::Kind::AsteriskAsteriskToken,
            None,
            "(ll ** lr) ** r",
        ),
        (
            None,
            ast::Kind::AsteriskToken,
            Some(ast::Kind::AsteriskToken),
            "l * rl * rr",
        ),
        (
            None,
            ast::Kind::BarToken,
            Some(ast::Kind::BarToken),
            "l | rl | rr",
        ),
        (
            None,
            ast::Kind::AmpersandToken,
            Some(ast::Kind::AmpersandToken),
            "l & rl & rr",
        ),
        (
            None,
            ast::Kind::CaretToken,
            Some(ast::Kind::CaretToken),
            "l ^ rl ^ rr",
        ),
        (
            None,
            ast::Kind::AmpersandAmpersandToken,
            Some(ast::Kind::ArrowFunction),
            "l && (() => { })",
        ),
    ];

    for (left_kind, operator_kind, right_kind, expected) in cases {
        let mut factory = ast::NodeFactory::default();
        let left = binary_side(&mut factory, "l", left_kind);
        let operator = factory.new_token(operator_kind);
        let right = binary_side(&mut factory, "r", right_kind);
        let expression = factory.new_binary_expression(None, left, None, operator, right);
        let file = source_file_from_expression(factory, expression);
        check_emit(file, &format!("{expected};"));
    }
}

#[test]
fn test_parenthesize_conditional_expression() {
    let cases: [(&str, fn(&mut ast::NodeFactory) -> ast::Node); 6] = [
        ("(a, b) ? c : d;", |factory| {
            let condition = binary(factory, "a", ast::Kind::CommaToken, "b");
            let question = factory.new_token(ast::Kind::QuestionToken);
            let when_true = factory.new_identifier("c");
            let colon = factory.new_token(ast::Kind::ColonToken);
            let when_false = factory.new_identifier("d");
            factory.new_conditional_expression(condition, question, when_true, colon, when_false)
        }),
        ("(a = b) ? c : d;", |factory| {
            let condition = binary(factory, "a", ast::Kind::EqualsToken, "b");
            let question = factory.new_token(ast::Kind::QuestionToken);
            let when_true = factory.new_identifier("c");
            let colon = factory.new_token(ast::Kind::ColonToken);
            let when_false = factory.new_identifier("d");
            factory.new_conditional_expression(condition, question, when_true, colon, when_false)
        }),
        ("(() => { }) ? a : b;", |factory| {
            let condition = empty_arrow_function(factory);
            let question = factory.new_token(ast::Kind::QuestionToken);
            let when_true = factory.new_identifier("a");
            let colon = factory.new_token(ast::Kind::ColonToken);
            let when_false = factory.new_identifier("b");
            factory.new_conditional_expression(condition, question, when_true, colon, when_false)
        }),
        ("(yield) ? a : b;", |factory| {
            let condition = factory.new_yield_expression(None, None);
            let question = factory.new_token(ast::Kind::QuestionToken);
            let when_true = factory.new_identifier("a");
            let colon = factory.new_token(ast::Kind::ColonToken);
            let when_false = factory.new_identifier("b");
            factory.new_conditional_expression(condition, question, when_true, colon, when_false)
        }),
        ("a ? (b, c) : d;", |factory| {
            let condition = factory.new_identifier("a");
            let question = factory.new_token(ast::Kind::QuestionToken);
            let when_true = binary(factory, "b", ast::Kind::CommaToken, "c");
            let colon = factory.new_token(ast::Kind::ColonToken);
            let when_false = factory.new_identifier("d");
            factory.new_conditional_expression(condition, question, when_true, colon, when_false)
        }),
        ("a ? b : (c, d);", |factory| {
            let condition = factory.new_identifier("a");
            let question = factory.new_token(ast::Kind::QuestionToken);
            let when_true = factory.new_identifier("b");
            let colon = factory.new_token(ast::Kind::ColonToken);
            let when_false = binary(factory, "c", ast::Kind::CommaToken, "d");
            factory.new_conditional_expression(condition, question, when_true, colon, when_false)
        }),
    ];

    for (expected, build) in cases {
        let mut factory = ast::NodeFactory::default();
        let expression = build(&mut factory);
        let file = source_file_from_expression(factory, expression);
        check_emit(file, expected);
    }
}

#[test]
fn test_parenthesize_yield_and_spread() {
    let cases: [(&str, fn(&mut ast::NodeFactory) -> ast::Node); 4] = [
        ("yield (a, b);", |factory| {
            let expression = binary(factory, "a", ast::Kind::CommaToken, "b");
            factory.new_yield_expression(None, expression)
        }),
        ("[...(a, b)];", |factory| {
            let expression = binary(factory, "a", ast::Kind::CommaToken, "b");
            let spread = factory.new_spread_element(expression);
            let elements = node_list(factory, [spread]);
            factory.new_array_literal_expression(elements, false)
        }),
        ("a(...(b, c));", |factory| {
            let callee = factory.new_identifier("a");
            let expression = binary(factory, "b", ast::Kind::CommaToken, "c");
            let spread = factory.new_spread_element(expression);
            let arguments = node_list(factory, [spread]);
            factory.new_call_expression(callee, None, None, arguments, ast::NodeFlags::NONE)
        }),
        ("new a(...(b, c));", |factory| {
            let callee = factory.new_identifier("a");
            let expression = binary(factory, "b", ast::Kind::CommaToken, "c");
            let spread = factory.new_spread_element(expression);
            let arguments = node_list(factory, [spread]);
            factory.new_new_expression(callee, None, arguments)
        }),
    ];

    for (expected, build) in cases {
        let mut factory = ast::NodeFactory::default();
        let expression = build(&mut factory);
        let file = source_file_from_expression(factory, expression);
        check_emit(file, expected);
    }
}

#[test]
fn test_parenthesize_assertion_like_and_non_null_expressions() {
    let cases: [(&str, fn(&mut ast::NodeFactory) -> ast::Node); 4] = [
        ("(a, b)<c>;", |factory| {
            let expression = binary(factory, "a", ast::Kind::CommaToken, "b");
            let ty = type_ref(factory, "c");
            let type_arguments = node_list(factory, [ty]);
            factory.new_expression_with_type_arguments(expression, type_arguments)
        }),
        ("(a, b) as c;", |factory| {
            let expression = binary(factory, "a", ast::Kind::CommaToken, "b");
            let ty = type_ref(factory, "c");
            factory.new_as_expression(expression, ty)
        }),
        ("(a, b) satisfies c;", |factory| {
            let expression = binary(factory, "a", ast::Kind::CommaToken, "b");
            let ty = type_ref(factory, "c");
            factory.new_satisfies_expression(expression, ty)
        }),
        ("(a, b)!;", |factory| {
            let expression = binary(factory, "a", ast::Kind::CommaToken, "b");
            factory.new_non_null_expression(expression, ast::NodeFlags::NONE)
        }),
    ];

    for (expected, build) in cases {
        let mut factory = ast::NodeFactory::default();
        let expression = build(&mut factory);
        let file = source_file_from_expression(factory, expression);
        check_emit(file, expected);
    }
}

#[test]
fn test_parenthesize_expression_statement() {
    let cases: [(&str, fn(&mut ast::NodeFactory) -> ast::Node); 3] = [
        ("({});", |factory| {
            let properties = node_list(factory, []);
            factory.new_object_literal_expression(properties, false)
        }),
        ("(function () { });", empty_function_expression),
        ("class {\n};", empty_class_expression),
    ];

    for (expected, build) in cases {
        let mut factory = ast::NodeFactory::default();
        let expression = build(&mut factory);
        let file = source_file_from_expression(factory, expression);
        check_emit(file, expected);
    }
}

#[test]
fn test_parenthesize_export_default_expression() {
    let cases: [(&str, fn(&mut ast::NodeFactory) -> ast::Node); 3] = [
        ("export default (class {\n});", empty_class_expression),
        (
            "export default (function () { });",
            empty_function_expression,
        ),
        ("export default (a, b);", |factory| {
            binary(factory, "a", ast::Kind::CommaToken, "b")
        }),
    ];

    for (expected, build) in cases {
        let mut factory = ast::NodeFactory::default();
        let expression = build(&mut factory);
        let export = factory.new_export_assignment(None, false, None, expression);
        let file = source_file_from_statements(factory, [export]);
        check_emit(file, expected);
    }
}

#[test]
fn test_parenthesize_type_postfix_and_operator_nodes() {
    let cases: [(&str, fn(&mut ast::NodeFactory) -> ast::Node); 10] = [
        ("type _ = (a | b)[];", |factory| {
            let union = named_union_type(factory, &["a", "b"]);
            factory.new_array_type_node(union)
        }),
        ("type _ = [\n    (a | b)?\n];", |factory| {
            let union = named_union_type(factory, &["a", "b"]);
            let optional = factory.new_optional_type_node(union);
            let elements = node_list(factory, [optional]);
            factory.new_tuple_type_node(elements)
        }),
        ("type _ = a | (() => b);", |factory| {
            let a = type_ref(factory, "a");
            let b = type_ref(factory, "b");
            let function = function_type(factory, b);
            let types = node_list(factory, [a, function]);
            factory.new_union_type_node(types)
        }),
        ("type _ = (infer a extends b) | c;", |factory| {
            let infer = constrained_infer_type(factory, "a", "b");
            let c = type_ref(factory, "c");
            let types = node_list(factory, [infer, c]);
            factory.new_union_type_node(types)
        }),
        ("type _ = a & (b | c);", |factory| {
            let a = type_ref(factory, "a");
            let union = named_union_type(factory, &["b", "c"]);
            let types = node_list(factory, [a, union]);
            factory.new_intersection_type_node(types)
        }),
        ("type _ = readonly (a | b);", |factory| {
            let union = named_union_type(factory, &["a", "b"]);
            factory.new_type_operator_node(ast::Kind::ReadonlyKeyword, union)
        }),
        ("type _ = readonly (keyof a);", |factory| {
            let a = type_ref(factory, "a");
            let keyof = factory.new_type_operator_node(ast::Kind::KeyOfKeyword, a);
            factory.new_type_operator_node(ast::Kind::ReadonlyKeyword, keyof)
        }),
        ("type _ = keyof (a | b);", |factory| {
            let union = named_union_type(factory, &["a", "b"]);
            factory.new_type_operator_node(ast::Kind::KeyOfKeyword, union)
        }),
        ("type _ = (a | b)[c];", |factory| {
            let union = named_union_type(factory, &["a", "b"]);
            let c = type_ref(factory, "c");
            factory.new_indexed_access_type_node(union, c)
        }),
        ("type _ = (typeof a)[b];", |factory| {
            let a = factory.new_identifier("a");
            let query = factory.new_type_query_node(a, None);
            let b = type_ref(factory, "b");
            factory.new_indexed_access_type_node(query, b)
        }),
    ];

    for (expected, build) in cases {
        let mut factory = ast::NodeFactory::default();
        let type_node = build(&mut factory);
        let file = type_alias_file(factory, type_node);
        check_emit(file, expected);
    }
}

#[test]
fn test_parenthesize_conditional_type() {
    let cases: [(&str, fn(&mut ast::NodeFactory) -> ast::Node); 4] = [
        ("type _ = (() => a) extends b ? c : d;", |factory| {
            let a = type_ref(factory, "a");
            let check = function_type(factory, a);
            let extends = type_ref(factory, "b");
            let true_type = type_ref(factory, "c");
            let false_type = type_ref(factory, "d");
            factory.new_conditional_type_node(check, extends, true_type, false_type)
        }),
        (
            "type _ = a extends (b extends c ? d : e) ? f : g;",
            |factory| {
                let a = type_ref(factory, "a");
                let b = type_ref(factory, "b");
                let c = type_ref(factory, "c");
                let d = type_ref(factory, "d");
                let e = type_ref(factory, "e");
                let extends = factory.new_conditional_type_node(b, c, d, e);
                let true_type = type_ref(factory, "f");
                let false_type = type_ref(factory, "g");
                factory.new_conditional_type_node(a, extends, true_type, false_type)
            },
        ),
        (
            "type _ = a extends () => (infer b extends c) ? d : e;",
            |factory| {
                let a = type_ref(factory, "a");
                let infer = constrained_infer_type(factory, "b", "c");
                let extends = function_type(factory, infer);
                let true_type = type_ref(factory, "d");
                let false_type = type_ref(factory, "e");
                factory.new_conditional_type_node(a, extends, true_type, false_type)
            },
        ),
        (
            "type _ = a extends () => (infer b extends c) | d ? e : f;",
            |factory| {
                let a = type_ref(factory, "a");
                let infer = constrained_infer_type(factory, "b", "c");
                let d = type_ref(factory, "d");
                let union_types = node_list(factory, [infer, d]);
                let union = factory.new_union_type_node(union_types);
                let extends = function_type(factory, union);
                let true_type = type_ref(factory, "e");
                let false_type = type_ref(factory, "f");
                factory.new_conditional_type_node(a, extends, true_type, false_type)
            },
        ),
    ];

    for (expected, build) in cases {
        let mut factory = ast::NodeFactory::default();
        let type_node = build(&mut factory);
        let file = type_alias_file(factory, type_node);
        check_emit(file, expected);
    }
}

#[test]
fn test_name_generation() {
    let mut emit_context = crate::new_emit_context();
    let temp1 = emit_context.factory.new_temp_variable();
    let declaration1 = emit_context
        .factory
        .new_variable_declaration(temp1, None, None, None);
    let declarations1 = emit_context.factory.new_node_list([declaration1]);
    let declaration_list1 = emit_context
        .factory
        .new_variable_declaration_list(declarations1, ast::NodeFlags::NONE);
    let statement1 = emit_context
        .factory
        .new_variable_statement(None, declaration_list1);

    let temp2 = emit_context.factory.new_temp_variable();
    let declaration2 = emit_context
        .factory
        .new_variable_declaration(temp2, None, None, None);
    let declarations2 = emit_context.factory.new_node_list([declaration2]);
    let declaration_list2 = emit_context
        .factory
        .new_variable_declaration_list(declarations2, ast::NodeFlags::NONE);
    let statement2 = emit_context
        .factory
        .new_variable_statement(None, declaration_list2);
    let block_statements = emit_context.factory.new_node_list([statement2]);
    let body = emit_context.factory.new_block(block_statements, true);

    let function_name = emit_context.factory.new_identifier("f");
    let parameters = emit_context.factory.new_node_list([]);
    let function = emit_context.factory.new_function_declaration(
        None,
        None,
        function_name,
        None,
        parameters,
        None,
        None,
        body,
    );

    let statements = emit_context.factory.new_node_list([statement1, function]);
    let eof = emit_context.factory.new_token(ast::Kind::EndOfFile);
    let file = emit_context.factory.new_source_file(
        ast::SourceFileParseOptions {
            file_name: "/file.ts".to_string(),
            path: "/file.ts".to_string(),
            ..Default::default()
        },
        "",
        statements,
        eof,
    );

    check_emit_with_context(
        file,
        emit_context,
        "var _a;\nfunction f() {\n    var _a;\n}",
    );
}

#[test]
fn test_external_helper_name_substitution() {
    let mut emit_context = crate::new_emit_context();
    let file = parse_typescript("__decorate", false);
    let external_helpers_module_name = emit_context.factory.new_identifier("tslib_1");
    let statement = file
        .statements_view()
        .iter()
        .next()
        .expect("test source should have one statement");
    let helper = file
        .store()
        .expression(statement)
        .expect("test statement should be an expression statement");

    emit_context.set_source_file(Some(&file));
    emit_context.set_emit_flags(&helper, crate::EF_HELPER_NAME);
    emit_context.set_external_helpers_module_name(&file, &external_helpers_module_name);

    check_emit_with_context(file, emit_context, "tslib_1.__decorate;");
}

#[test]
fn test_parenthesize_binary_expression_mixing_nullish_coalescing() {
    let cases = [
        (
            "BarBarWithLeftQuestionQuestion",
            ast::Kind::QuestionQuestionToken,
            ast::Kind::BarBarToken,
            "left",
            "(a ?? b) || c;",
        ),
        (
            "AmpersandAmpersandWithLeftQuestionQuestion",
            ast::Kind::QuestionQuestionToken,
            ast::Kind::AmpersandAmpersandToken,
            "left",
            "(a ?? b) && c;",
        ),
        (
            "BarBarWithRightQuestionQuestion",
            ast::Kind::QuestionQuestionToken,
            ast::Kind::BarBarToken,
            "right",
            "a || (b ?? c);",
        ),
        (
            "AmpersandAmpersandWithRightQuestionQuestion",
            ast::Kind::QuestionQuestionToken,
            ast::Kind::AmpersandAmpersandToken,
            "right",
            "a && (b ?? c);",
        ),
        (
            "QuestionQuestionWithLeftBarBar",
            ast::Kind::BarBarToken,
            ast::Kind::QuestionQuestionToken,
            "left",
            "(a || b) ?? c;",
        ),
        (
            "QuestionQuestionWithLeftAmpersandAmpersand",
            ast::Kind::AmpersandAmpersandToken,
            ast::Kind::QuestionQuestionToken,
            "left",
            "(a && b) ?? c;",
        ),
        (
            "QuestionQuestionWithRightBarBar",
            ast::Kind::BarBarToken,
            ast::Kind::QuestionQuestionToken,
            "right",
            "a ?? (b || c);",
        ),
        (
            "QuestionQuestionWithRightAmpersandAmpersand",
            ast::Kind::AmpersandAmpersandToken,
            ast::Kind::QuestionQuestionToken,
            "right",
            "a ?? (b && c);",
        ),
    ];

    for (title, inner_op, outer_op, side, expected) in cases {
        let mut factory = ast::NodeFactory::default();
        let (inner_left, inner_right, outer_left, outer_right) = if side == "left" {
            ("a", "b", None, Some("c"))
        } else {
            ("b", "c", Some("a"), None)
        };
        let inner = {
            let left = factory.new_identifier(inner_left);
            let operator = factory.new_token(inner_op);
            let right = factory.new_identifier(inner_right);
            factory.new_binary_expression(None, left, None, operator, right)
        };
        let outer = if let Some(left_text) = outer_left {
            let left = factory.new_identifier(left_text);
            let operator = factory.new_token(outer_op);
            factory.new_binary_expression(None, left, None, operator, inner)
        } else {
            let operator = factory.new_token(outer_op);
            let right = factory.new_identifier(outer_right.expect("right operand"));
            factory.new_binary_expression(None, inner, None, operator, right)
        };
        let statement = factory.new_expression_statement(outer);
        let statements = factory.new_node_list(
            core::undefined_text_range(),
            core::undefined_text_range(),
            [statement],
        );
        let eof = factory.new_token(ast::Kind::EndOfFile);
        let root = factory.new_source_file(
            ast::SourceFileParseOptions {
                file_name: "/file.ts".to_string(),
                path: "/file.ts".to_string(),
                ..Default::default()
            },
            "",
            statements,
            eof,
        );
        let file =
            factory.finish_parsed_source_file(root, ast::ParsedSourceFileMetadata::default());
        let mut printer = new_printer(PrinterOptions::default(), PrintHandlers::default(), None);
        let actual = file.emit_with(&mut printer);
        assert_eq!(expected, trim_emit_final_newline(&actual), "{title}");
    }
}
