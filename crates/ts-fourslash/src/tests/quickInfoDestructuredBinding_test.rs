use crate::{new_fourslash, TestingT};

pub fn test_quick_info_destructured_binding(t: &mut TestingT) {
    let content = r#"
function f({ /*1*/x }: { x: number }) {}
function g([/*2*/y]: number[]) {}
function h({ a: { /*3*/b } }: { a: { b: string } }) {}
const { /*4*/c } = { c: 42 };
let { /*5*/d } = { d: "hello" };
var { /*6*/e } = { e: true };
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    // Destructured object binding parameters should show "(parameter)" not "var"
    f.verify_quick_info_at(t, "1", "(parameter) x: number", "");
    // Destructured array binding parameters should show "(parameter)" not "var"
    f.verify_quick_info_at(t, "2", "(parameter) y: number", "");
    // Nested destructured parameters should also show "(parameter)"
    f.verify_quick_info_at(t, "3", "(parameter) b: string", "");
    // Destructured const/let/var bindings should show their proper keyword
    f.verify_quick_info_at(t, "4", "const c: number", "");
    f.verify_quick_info_at(t, "5", "let d: string", "");
    f.verify_quick_info_at(t, "6", "var e: boolean", "");
    done();
}

