#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_switch_case6() {
    let mut t = TestingT;
    run_test_go_to_definition_switch_case6(&mut t);
}

fn run_test_go_to_definition_switch_case6(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionSwitchCase6") {
        return;
    }
    let content = r"export default { [|/*a*/case|] };
[|/*b*/default|];
[|/*c*/case|] 42;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["a".to_string(), "b".to_string(), "c".to_string()]);
    done();
}
