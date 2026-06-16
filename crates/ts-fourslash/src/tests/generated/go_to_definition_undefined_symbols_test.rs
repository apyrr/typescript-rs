#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_undefined_symbols() {
    let mut t = TestingT;
    run_test_go_to_definition_undefined_symbols(&mut t);
}

fn run_test_go_to_definition_undefined_symbols(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionUndefinedSymbols") {
        return;
    }
    let content = r"some/*undefinedValue*/Variable;
var a: some/*undefinedType*/Type;
var x = {}; x.some/*undefinedProperty*/Property;
var a: any; a.some/*unkownProperty*/Property;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &f.marker_names());
    done();
}
