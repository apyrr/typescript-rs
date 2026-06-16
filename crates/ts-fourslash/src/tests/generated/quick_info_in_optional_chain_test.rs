#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_in_optional_chain() {
    let mut t = TestingT;
    run_test_quick_info_in_optional_chain(&mut t);
}

fn run_test_quick_info_in_optional_chain(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @strict: true
interface A {
  arr: string[];
}

function test(a?: A): string {
  return a?.ar/*1*/r.length ? "A" : "B";
}

interface Foo { bar: { baz: string } };
declare const foo: Foo | undefined;

if (foo?.b/*2*/ar.b/*3*/az) {}

interface Foo2 { bar?: { baz: { qwe: string } } };
declare const foo2: Foo2;

if (foo2.b/*4*/ar?.b/*5*/az.q/*6*/we) {}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(property) A.arr: string[]", "");
    f.verify_quick_info_at(t, "2", "(property) Foo.bar: {\n    baz: string;\n}", "");
    f.verify_quick_info_at(t, "3", "(property) baz: string | undefined", "");
    f.verify_quick_info_at(
        t,
        "4",
        "(property) Foo2.bar?: {\n    baz: {\n        qwe: string;\n    };\n} | undefined",
        "",
    );
    f.verify_quick_info_at(t, "5", "(property) baz: {\n    qwe: string;\n}", "");
    f.verify_quick_info_at(t, "6", "(property) qwe: string | undefined", "");
    done();
}
