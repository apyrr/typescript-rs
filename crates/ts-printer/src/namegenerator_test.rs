use ts_ast as ast;
use ts_core as core;
use ts_parser as parser;
use ts_tspath as tspath;

use crate::{
    AutoGenerateOptions, GeneratedIdentifierFlags, NameGenerator, NodeFactory, new_emit_context,
    new_node_factory,
};

fn new_generator(context: &mut crate::EmitContext) -> NameGenerator {
    let mut generator = NameGenerator::default();
    generator.context = Some(context.state_ref());
    generator
}

fn generate_name(g: &mut NameGenerator, factory: &NodeFactory, name: &ast::Node) -> String {
    g.generate_name(factory.store(), name)
}

fn generate_name_with_source_file(
    g: &mut NameGenerator,
    factory: &NodeFactory,
    file: &ast::SourceFile,
    name: &ast::Node,
) -> String {
    let binding_state = ts_binder::bind_source_file(file);
    let factory_store = factory.store();
    g.generate_name_with_resolver_and_binding_facts(
        factory_store,
        name,
        |node| {
            if node.store_id() == factory_store.store_id() {
                factory_store
            } else {
                file.store()
            }
        },
        Some(binding_state.as_ref()),
    )
}

fn source_generated_name_for_node(
    factory: &mut NodeFactory,
    source: &ast::AstStore,
    node: ast::Node,
) -> ast::Node {
    factory.new_generated_name_for_node(source, &node)
}

fn source_generated_name_for_node_ex(
    factory: &mut NodeFactory,
    source: &ast::AstStore,
    node: ast::Node,
    options: AutoGenerateOptions,
) -> ast::Node {
    factory.new_generated_name_for_node_ex(source, &node, options)
}

fn source_generated_private_name_for_node(
    factory: &mut NodeFactory,
    source: &ast::AstStore,
    node: ast::Node,
) -> ast::Node {
    factory.new_generated_private_name_for_node(source, &node)
}

fn statement(file: &ast::SourceFile, index: usize) -> ast::Node {
    file.statements_view()
        .into_iter()
        .nth(index)
        .expect("statement")
}

fn statement_in_body(store: &ast::AstStore, node: ast::Node, index: usize) -> ast::Node {
    let body = store.body(node).expect("body");
    store
        .source_statements(body)
        .expect("statements")
        .into_iter()
        .nth(index)
        .expect("statement")
}

fn member(store: &ast::AstStore, node: ast::Node, index: usize) -> ast::Node {
    store
        .members(node)
        .expect("members")
        .into_iter()
        .nth(index)
        .expect("member")
}

fn parse_type_script(text: &str, jsx: bool) -> ast::SourceFile {
    let file_name = if jsx { "/main.tsx" } else { "/main.ts" };
    parser::parse_source_file(
        ast::SourceFileParseOptions {
            file_name: file_name.to_string(),
            path: file_name.to_string(),
            ..Default::default()
        },
        text.to_string(),
        if jsx {
            core::ScriptKind::TSX
        } else {
            core::ScriptKind::TS
        },
    )
}

#[test]
fn test_temp_variable_1() {
    let mut ec = new_emit_context();
    let mut factory = new_node_factory(&mut ec);
    let name1 = factory.new_temp_variable();
    let name2 = factory.new_temp_variable();

    let mut g = new_generator(&mut ec);
    let text1 = generate_name(&mut g, &factory, &name1);
    let text2 = generate_name(&mut g, &factory, &name2);

    assert_eq!("_a", text1);
    assert_eq!("_b", text2);
}

#[test]
fn test_temp_variable_2() {
    let mut ec = new_emit_context();
    let mut factory = new_node_factory(&mut ec);
    let name1 = factory.new_temp_variable_ex(AutoGenerateOptions {
        prefix: "A",
        suffix: "B",
        ..Default::default()
    });
    let name2 = factory.new_temp_variable_ex(AutoGenerateOptions {
        prefix: "A",
        suffix: "B",
        ..Default::default()
    });

    let mut g = new_generator(&mut ec);
    let text1 = generate_name(&mut g, &factory, &name1);
    let text2 = generate_name(&mut g, &factory, &name2);

    assert_eq!("A_aB", text1);
    assert_eq!("A_bB", text2);
}

#[test]
fn test_temp_variable_3() {
    let mut ec = new_emit_context();
    let mut factory = new_node_factory(&mut ec);
    let name1 = factory.new_temp_variable();

    let mut g = new_generator(&mut ec);
    let text1 = generate_name(&mut g, &factory, &name1);
    let text2 = generate_name(&mut g, &factory, &name1);

    assert_eq!("_a", text1);
    assert_eq!("_a", text2);
}

#[test]
fn test_temp_variable_scoped() {
    let mut ec = new_emit_context();
    let mut factory = new_node_factory(&mut ec);
    let name1 = factory.new_temp_variable();
    let name2 = factory.new_temp_variable();

    let mut g = new_generator(&mut ec);
    let text1 = generate_name(&mut g, &factory, &name1);
    g.push_scope(false);
    let text2 = generate_name(&mut g, &factory, &name2);
    g.pop_scope(false);

    assert_eq!("_a", text1);
    assert_eq!("_a", text2);
}

#[test]
fn test_temp_variable_scoped_reserved() {
    let mut ec = new_emit_context();
    let mut factory = new_node_factory(&mut ec);
    let name1 = factory.new_temp_variable_ex(AutoGenerateOptions {
        flags: GeneratedIdentifierFlags::RESERVED_IN_NESTED_SCOPES,
        ..Default::default()
    });
    let name2 = factory.new_temp_variable();

    let mut g = new_generator(&mut ec);
    let text1 = generate_name(&mut g, &factory, &name1);
    g.push_scope(false);
    let text2 = generate_name(&mut g, &factory, &name2);
    g.pop_scope(false);

    assert_eq!("_a", text1);
    assert_eq!("_b", text2);
}

#[test]
fn test_loop_variable_1() {
    let mut ec = new_emit_context();
    let mut factory = new_node_factory(&mut ec);
    let name1 = factory.new_loop_variable();
    let name2 = factory.new_loop_variable();

    let mut g = new_generator(&mut ec);
    let text1 = generate_name(&mut g, &factory, &name1);
    let text2 = generate_name(&mut g, &factory, &name2);

    assert_eq!("_i", text1);
    assert_eq!("_a", text2);
}

#[test]
fn test_loop_variable_2() {
    let mut ec = new_emit_context();
    let mut factory = new_node_factory(&mut ec);
    let name1 = factory.new_loop_variable_ex(AutoGenerateOptions {
        prefix: "A",
        suffix: "B",
        ..Default::default()
    });
    let name2 = factory.new_loop_variable_ex(AutoGenerateOptions {
        prefix: "A",
        suffix: "B",
        ..Default::default()
    });

    let mut g = new_generator(&mut ec);
    let text1 = generate_name(&mut g, &factory, &name1);
    let text2 = generate_name(&mut g, &factory, &name2);

    assert_eq!("A_iB", text1);
    assert_eq!("A_aB", text2);
}

#[test]
fn test_loop_variable_3() {
    let mut ec = new_emit_context();
    let mut factory = new_node_factory(&mut ec);
    let name1 = factory.new_loop_variable();

    let mut g = new_generator(&mut ec);
    let text1 = generate_name(&mut g, &factory, &name1);
    let text2 = generate_name(&mut g, &factory, &name1);

    assert_eq!("_i", text1);
    assert_eq!("_i", text2);
}

#[test]
fn test_loop_variable_scoped() {
    let mut ec = new_emit_context();
    let mut factory = new_node_factory(&mut ec);
    let name1 = factory.new_loop_variable();
    let name2 = factory.new_loop_variable();

    let mut g = new_generator(&mut ec);
    let text1 = generate_name(&mut g, &factory, &name1);
    g.push_scope(false);
    let text2 = generate_name(&mut g, &factory, &name2);
    g.pop_scope(false);

    assert_eq!("_i", text1);
    assert_eq!("_i", text2);
}

#[test]
fn test_unique_name_1() {
    let mut ec = new_emit_context();
    let mut factory = new_node_factory(&mut ec);
    let name1 = factory.new_unique_name("foo");
    let name2 = factory.new_unique_name("foo");

    let mut g = new_generator(&mut ec);
    let text1 = generate_name(&mut g, &factory, &name1);
    let text2 = generate_name(&mut g, &factory, &name2);

    assert_eq!("foo_1", text1);
    assert_eq!("foo_2", text2);
}

#[test]
fn test_unique_name_2() {
    let mut ec = new_emit_context();
    let mut factory = new_node_factory(&mut ec);
    let name1 = factory.new_unique_name("foo");

    let mut g = new_generator(&mut ec);
    let text1 = generate_name(&mut g, &factory, &name1);
    let text2 = generate_name(&mut g, &factory, &name1);

    assert_eq!("foo_1", text1);
    // Expected to be same because GenerateName goes off object identity
    assert_eq!("foo_1", text2);
}

#[test]
fn test_unique_name_scoped() {
    let mut ec = new_emit_context();
    let mut factory = new_node_factory(&mut ec);
    let name1 = factory.new_unique_name("foo");
    let name2 = factory.new_unique_name("foo");

    let mut g = new_generator(&mut ec);
    assert_eq!("foo_1", generate_name(&mut g, &factory, &name1));

    g.push_scope(false);
    assert_eq!("foo_2", generate_name(&mut g, &factory, &name2)); // Matches Strada, but is incorrect
    g.pop_scope(false);
}

#[test]
fn test_unique_private_name_1() {
    let mut ec = new_emit_context();
    let mut factory = new_node_factory(&mut ec);
    let name1 = factory.new_unique_private_name("#foo");
    let name2 = factory.new_unique_private_name("#foo");

    let mut g = new_generator(&mut ec);
    let text1 = generate_name(&mut g, &factory, &name1);
    let text2 = generate_name(&mut g, &factory, &name2);

    assert_eq!("#foo_1", text1);
    assert_eq!("#foo_2", text2);
}

#[test]
fn test_unique_private_name_2() {
    let mut ec = new_emit_context();
    let mut factory = new_node_factory(&mut ec);
    let name1 = factory.new_unique_private_name("#foo");

    let mut g = new_generator(&mut ec);
    let text1 = generate_name(&mut g, &factory, &name1);
    let text2 = generate_name(&mut g, &factory, &name1);

    assert_eq!("#foo_1", text1);
    assert_eq!("#foo_1", text2);
}

#[test]
fn test_unique_private_name_scoped() {
    let mut ec = new_emit_context();
    let mut factory = new_node_factory(&mut ec);
    let name1 = factory.new_unique_private_name("#foo");
    let name2 = factory.new_unique_private_name("#foo");

    let mut g = new_generator(&mut ec);
    assert_eq!("#foo_1", generate_name(&mut g, &factory, &name1));

    g.push_scope(false); // private names are always reserved in nested scopes
    assert_eq!("#foo_2", generate_name(&mut g, &factory, &name2));
    g.pop_scope(false);
}

#[test]
fn test_generated_name_for_identifier_1() {
    let mut ec = new_emit_context();

    let file = parse_type_script("function f() {}", false);

    let n = file.store().name(statement(&file, 0)).unwrap();
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node(&mut factory, file.store(), n);

    let mut g = new_generator(&mut ec);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name1);

    assert_eq!("f_1", text1);
}

#[test]
fn test_generated_name_for_identifier_2() {
    let mut ec = new_emit_context();

    let file = parse_type_script("function f() {}", false);

    let n = file.store().name(statement(&file, 0)).unwrap();
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node_ex(
        &mut factory,
        file.store(),
        n,
        AutoGenerateOptions {
            prefix: "a",
            suffix: "b",
            ..Default::default()
        },
    );

    let mut g = new_generator(&mut ec);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name1);

    assert_eq!("afb", text1);
}

#[test]
fn test_generated_name_for_identifier_3() {
    let mut ec = new_emit_context();

    let file = parse_type_script("function f() {}", false);

    let n = file.store().name(statement(&file, 0)).unwrap();
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node_ex(
        &mut factory,
        file.store(),
        n,
        AutoGenerateOptions {
            prefix: "a",
            suffix: "b",
            ..Default::default()
        },
    );
    let name2 = factory.new_generated_name_for_factory_node(&name1);

    let mut g = new_generator(&mut ec);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name2);

    assert_eq!("afb_1", text1);
}

// namespace reuses name if it does not collide with locals
#[test]
fn test_generated_name_for_namespace_1() {
    let mut ec = new_emit_context();

    let file = parse_type_script("namespace foo { }", false);

    let ns1 = statement(&file, 0);
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node(&mut factory, file.store(), ns1);

    let mut g = new_generator(&mut ec);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name1);

    assert_eq!("foo", text1);
}

// namespace uses generated name if it collides with locals
#[test]
fn test_generated_name_for_namespace_2() {
    let mut ec = new_emit_context();

    let file = parse_type_script("namespace foo { var foo; }", false);

    let ns1 = statement(&file, 0);
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node(&mut factory, file.store(), ns1);

    let mut g = new_generator(&mut ec);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name1);

    assert_eq!("foo_1", text1);
}

// avoids collisions when unscoped
#[test]
fn test_generated_name_for_namespace_3() {
    let mut ec = new_emit_context();

    let file = parse_type_script(
        "namespace ns1 { namespace foo { var foo; } } namespace ns2 { namespace foo { var foo; } }",
        false,
    );

    let statements = file.statements_view().into_iter().collect::<Vec<_>>();
    let ns1 = statement_in_body(file.store(), statements[0], 0);
    let ns2 = statement_in_body(file.store(), statements[1], 0);
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node(&mut factory, file.store(), ns1);
    let name2 = source_generated_name_for_node(&mut factory, file.store(), ns2);

    let mut g = new_generator(&mut ec);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name1);
    let text2 = generate_name_with_source_file(&mut g, &factory, &file, &name2);

    assert_eq!("foo_1", text1);
    assert_eq!("foo_2", text2);
}

// reuse name when scoped
#[test]
fn test_generated_name_for_namespace_4() {
    let mut ec = new_emit_context();

    let file = parse_type_script(
        "namespace ns1 { namespace foo { var foo; } } namespace ns2 { namespace foo { var foo; } }",
        false,
    );

    let statements = file.statements_view().into_iter().collect::<Vec<_>>();
    let ns1 = statement_in_body(file.store(), statements[0], 0);
    let ns2 = statement_in_body(file.store(), statements[1], 0);
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node(&mut factory, file.store(), ns1);
    let name2 = source_generated_name_for_node(&mut factory, file.store(), ns2);

    let mut g = new_generator(&mut ec);
    g.push_scope(false);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name1);
    g.pop_scope(false);

    g.push_scope(false);
    let text2 = generate_name_with_source_file(&mut g, &factory, &file, &name2);
    g.pop_scope(false);

    assert_eq!("foo_1", text1);
    assert_eq!("foo_2", text2); // Matches Strada, but is incorrect
}

#[test]
fn test_generated_name_for_node_cached() {
    let mut ec = new_emit_context();

    let file = parse_type_script("namespace foo { var foo; }", false);

    let ns1 = statement(&file, 0);
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node(&mut factory, file.store(), ns1);
    let name2 = source_generated_name_for_node(&mut factory, file.store(), ns1);

    let mut g = new_generator(&mut ec);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name1);
    let text2 = generate_name_with_source_file(&mut g, &factory, &file, &name2);

    assert_eq!("foo_1", text1);
    assert_eq!("foo_1", text2);
}

#[test]
fn test_generated_name_for_import() {
    let mut ec = new_emit_context();

    let file = parse_type_script("import * as foo from 'foo'", false);

    let n = statement(&file, 0);
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node(&mut factory, file.store(), n);

    let mut g = new_generator(&mut ec);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name1);

    assert_eq!("foo_1", text1);
}

#[test]
fn test_generated_name_for_export() {
    let mut ec = new_emit_context();

    let file = parse_type_script("export * as foo from 'foo'", false);

    let n = statement(&file, 0);
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node(&mut factory, file.store(), n);

    let mut g = new_generator(&mut ec);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name1);

    assert_eq!("foo_1", text1);
}

#[test]
fn test_generated_name_for_function_declaration_1() {
    let mut ec = new_emit_context();

    let file = parse_type_script("export function f() {}", false);

    let n = statement(&file, 0);
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node(&mut factory, file.store(), n);

    let mut g = new_generator(&mut ec);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name1);

    assert_eq!("f_1", text1);
}

#[test]
fn test_generated_name_for_function_declaration_2() {
    let mut ec = new_emit_context();

    let file = parse_type_script("export default function () {}", false);

    let n = statement(&file, 0);
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node(&mut factory, file.store(), n);

    let mut g = new_generator(&mut ec);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name1);

    assert_eq!("default_1", text1);
}

#[test]
fn test_generated_name_for_class_declaration_1() {
    let mut ec = new_emit_context();

    let file = parse_type_script("export class C {}", false);

    let n = statement(&file, 0);
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node(&mut factory, file.store(), n);

    let mut g = new_generator(&mut ec);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name1);

    assert_eq!("C_1", text1);
}

#[test]
fn test_generated_name_for_merged_namespace_declaration() {
    let mut ec = new_emit_context();

    let file_name = "/main.ts".to_owned();
    let path = tspath::to_path(&file_name, "/", true);
    let file = parser::parse_source_file_as_parsed(
        ast::SourceFileParseOptions {
            file_name,
            path,
            ..Default::default()
        },
        "namespace M { export class C {} export namespace C { export var C = M.C } }".to_owned(),
        core::ScriptKind::TS,
    );
    let binding_state = ts_binder::bind_parsed_source_file(&file);

    let outer_namespace = file
        .store()
        .parser_access()
        .source_file_statement_nodes(file.root())[0];
    let inner_namespace = statement_in_body(file.store(), outer_namespace, 1);
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node(&mut factory, file.store(), inner_namespace);

    let mut g = new_generator(&mut ec);
    let factory_store = factory.store();
    let text1 = g.generate_name_with_resolver_and_binding_facts(
        factory_store,
        &name1,
        |node| {
            if node.store_id() == factory_store.store_id() {
                factory_store
            } else {
                file.store()
            }
        },
        Some(binding_state.as_ref()),
    );

    assert_eq!("C_1", text1);
}

#[test]
fn test_generated_name_for_class_declaration_2() {
    let mut ec = new_emit_context();

    let file = parse_type_script("export default class {}", false);

    let n = statement(&file, 0);
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node(&mut factory, file.store(), n);

    let mut g = new_generator(&mut ec);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name1);

    assert_eq!("default_1", text1);
}

#[test]
fn test_generated_name_for_export_assignment() {
    let mut ec = new_emit_context();

    let file = parse_type_script("export default 0", false);

    let n = statement(&file, 0);
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node(&mut factory, file.store(), n);

    let mut g = new_generator(&mut ec);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name1);

    assert_eq!("default_1", text1);
}

#[test]
fn test_generated_name_for_class_expression() {
    let mut ec = new_emit_context();

    let file = parse_type_script("(class {})", false);

    let expression_statement = statement(&file, 0);
    let expression = file.store().expression(expression_statement).unwrap();
    let n = file.store().expression(expression).unwrap_or(expression);
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node(&mut factory, file.store(), n);

    let mut g = new_generator(&mut ec);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name1);

    assert_eq!("class_1", text1);
}

#[test]
fn test_generated_name_for_method_1() {
    let mut ec = new_emit_context();

    let file = parse_type_script("class C { m() {} }", false);

    let n = member(file.store(), statement(&file, 0), 0);
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node(&mut factory, file.store(), n);

    let mut g = new_generator(&mut ec);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name1);

    assert_eq!("m_1", text1);
}

#[test]
fn test_generated_name_for_method_2() {
    let mut ec = new_emit_context();

    let file = parse_type_script("class C { 0() {} }", false);

    let n = member(file.store(), statement(&file, 0), 0);
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node(&mut factory, file.store(), n);

    let mut g = new_generator(&mut ec);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name1);

    assert_eq!("_a", text1);
}

#[test]
fn test_generated_private_name_for_method() {
    let mut ec = new_emit_context();

    let file = parse_type_script("class C { m() {} }", false);

    let n = member(file.store(), statement(&file, 0), 0);
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_private_name_for_node(&mut factory, file.store(), n);

    let mut g = new_generator(&mut ec);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name1);

    assert_eq!("#m_1", text1);
}

#[test]
fn test_generated_name_for_computed_property_name() {
    let mut ec = new_emit_context();

    let file = parse_type_script("class C { [x] }", false);

    let member = member(file.store(), statement(&file, 0), 0);
    let n = file.store().name(member).unwrap();
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node(&mut factory, file.store(), n);

    let mut g = new_generator(&mut ec);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name1);

    assert_eq!("_a", text1);
}

#[test]
fn test_generated_name_for_other() {
    let mut ec = new_emit_context();

    let file = parse_type_script("({})", false);

    let expression_statement = statement(&file, 0);
    let expression = file.store().expression(expression_statement).unwrap();
    let n = file.store().expression(expression).unwrap_or(expression);
    let mut factory = new_node_factory(&mut ec);
    let name1 = source_generated_name_for_node(&mut factory, file.store(), n);

    let mut g = new_generator(&mut ec);
    let text1 = generate_name_with_source_file(&mut g, &factory, &file, &name1);

    assert_eq!("_a", text1);
}
