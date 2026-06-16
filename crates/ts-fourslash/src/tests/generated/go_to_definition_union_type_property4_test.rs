#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_union_type_property4() {
    let mut t = TestingT;
    run_test_go_to_definition_union_type_property4(&mut t);
}

fn run_test_go_to_definition_union_type_property4(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionUnionTypeProperty4") {
        return;
    }
    let content = r"interface SnapCrackle {
    /*def1*/pop(): string;
}

interface Magnitude {
    /*def2*/pop(): number;
}

interface Art {
    /*def3*/pop(): boolean;
}

var art: Art;
var magnitude: Magnitude;
var snapcrackle: SnapCrackle;

var x = (snapcrackle || magnitude || art).[|/*usage*/pop|];";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["usage".to_string()]);
    done();
}
