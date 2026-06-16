#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_typedef_tag_go_to_definition() {
    let mut t = TestingT;
    run_test_jsdoc_typedef_tag_go_to_definition(&mut t);
}

fn run_test_jsdoc_typedef_tag_go_to_definition(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocTypedefTagGoToDefinition") {
        return;
    }
    let content = r"// @lib: es5
// @allowNonTsExtensions: true
// @Filename: jsdocCompletion_typedef.js
/**
 * @typedef {Object} Person
 * @property {string} /*1*/personName
 * @property {number} personAge
 */

/**
 * @typedef {{ /*2*/animalName: string, animalAge: number }} Animal
 */

/** @type {Person} */
var person; person.[|personName/*3*/|]

/** @type {Animal} */
var animal; animal.[|animalName/*4*/|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_go_to_definition(t, &["3".to_string(), "4".to_string()]);
    done();
}
