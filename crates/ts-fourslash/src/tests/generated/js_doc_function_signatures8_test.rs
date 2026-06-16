#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_js_doc_function_signatures8() {
    let mut t = TestingT;
    run_test_js_doc_function_signatures8(&mut t);
}

fn run_test_js_doc_function_signatures8(t: &mut TestingT) {
    if should_skip_if_failing("TestJsDocFunctionSignatures8") {
        return;
    }
    let content = r"// @allowJs: true
// @Filename: Foo.js
/**
 * Represents a person
 * a b multiline test
 * @constructor
 * @param {string} name The name of the person
 * @param {number} age The age of the person
 */
function Person(name, age) {
    this.name = name;
    this.age = age;
}
var p = new Pers/**/on();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_quick_info_is(
        t,
        "constructor Person(name: string, age: number): Person",
        "Represents a person\na b multiline test",
    );
    done();
}
