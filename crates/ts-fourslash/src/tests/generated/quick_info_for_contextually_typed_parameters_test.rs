#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_for_contextually_typed_parameters() {
    let mut t = TestingT;
    run_test_quick_info_for_contextually_typed_parameters(&mut t);
}

fn run_test_quick_info_for_contextually_typed_parameters(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoForContextuallyTypedParameters") {
        return;
    }
    let content = r#"declare function foo1<T>(obj: T, settings: (row: T) => { value: string, func?: Function }): void;

foo1(new Error(),
    o/*1*/ => ({
        value: o.name,
        func: x => 'foo'
    })
);

declare function foo2<T>(settings: (row: T) => { value: string, func?: Function }, obj: T): void;

foo2(o/*2*/ => ({
        value: o.name,
        func: x => 'foo'
    }),
    new Error(),
);

declare function foof<T extends { name: string }, U extends keyof T>(settings: (row: T) => { value: T[U], func?: Function }, obj: T, key: U): U;

function q<T extends { name: string }>(x: T): T["name"] {
    return foof/*3*/(o => ({ value: o.name, func: x => 'foo' }), x, "name");
}

foof/*4*/(o => ({ value: o.name, func: x => 'foo' }), new Error(), "name");"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(parameter) o: Error", "");
    f.verify_quick_info_at(t, "2", "(parameter) o: Error", "");
    f.verify_quick_info_at(t, "3", "function foof<T, \"name\">(settings: (row: T) => {\n    value: T[\"name\"];\n    func?: Function;\n}, obj: T, key: \"name\"): \"name\"", "");
    f.verify_quick_info_at(t, "4", "function foof<Error, \"name\">(settings: (row: Error) => {\n    value: string;\n    func?: Function;\n}, obj: Error, key: \"name\"): \"name\"", "");
    done();
}
