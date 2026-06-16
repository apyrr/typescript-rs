#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_generic_tagged_template_expression() {
    let mut t = TestingT;
    run_test_quick_info_for_generic_tagged_template_expression(&mut t);
}

fn run_test_quick_info_for_generic_tagged_template_expression(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForGenericTaggedTemplateExpression") {
        return;
    }
    let content = r#"interface T1 {}
class T2 {}
type T3 = "a" | "b";

declare function foo<T>(strings: TemplateStringsArray, ...values: T[]): void;

/*1*/foo<number>``;
/*2*/foo<string | number>``;
/*3*/foo<{ a: number }>``;
/*4*/foo<T1>``;
/*5*/foo<T2>``;
/*6*/foo<T3>``;
/*7*/foo``;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "1",
        "function foo<number>(strings: TemplateStringsArray, ...values: number[]): void",
        "",
    );
    f.verify_quick_info_at(t, "2", "function foo<string | number>(strings: TemplateStringsArray, ...values: (string | number)[]): void", "");
    f.verify_quick_info_at(t, "3", "function foo<{\n    a: number;\n}>(strings: TemplateStringsArray, ...values: {\n    a: number;\n}[]): void", "");
    f.verify_quick_info_at(
        t,
        "4",
        "function foo<T1>(strings: TemplateStringsArray, ...values: T1[]): void",
        "",
    );
    f.verify_quick_info_at(
        t,
        "5",
        "function foo<T2>(strings: TemplateStringsArray, ...values: T2[]): void",
        "",
    );
    f.verify_quick_info_at(
        t,
        "6",
        "function foo<T3>(strings: TemplateStringsArray, ...values: T3[]): void",
        "",
    );
    f.verify_quick_info_at(
        t,
        "7",
        "function foo<unknown>(strings: TemplateStringsArray, ...values: unknown[]): void",
        "",
    );
    done();
}
