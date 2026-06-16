#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_built_in_values() {
    let mut t = TestingT;
    run_test_go_to_definition_built_in_values(&mut t);
}

fn run_test_go_to_definition_built_in_values(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var u = /*undefined*/undefined;
var n = /*null*/null;
var a = function() { return /*arguments*/arguments; };
var t = /*true*/true;
var f = /*false*/false;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &f.marker_names());
    done();
}
