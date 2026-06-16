#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_goto_definition_throws_tag() {
    let mut t = TestingT;
    run_test_goto_definition_throws_tag(&mut t);
}

fn run_test_goto_definition_throws_tag(t: &mut TestingT) {
    if should_skip_if_failing("TestGotoDefinitionThrowsTag") {
        return;
    }
    let content = r"class [|/*def*/E|] extends Error {}

/**
 * @throws {/*use*/[|E|]}
 */
function f() {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["use".to_string()]);
    done();
}
