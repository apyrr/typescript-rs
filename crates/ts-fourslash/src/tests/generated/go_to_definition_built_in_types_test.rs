#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_built_in_types() {
    let mut t = TestingT;
    run_test_go_to_definition_built_in_types(&mut t);
}

fn run_test_go_to_definition_built_in_types(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionBuiltInTypes") {
        return;
    }
    let content = r"var n: /*number*/number;
var s: /*string*/string;
var b: /*boolean*/boolean;
var v: /*void*/void;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &f.marker_names());
    done();
}
