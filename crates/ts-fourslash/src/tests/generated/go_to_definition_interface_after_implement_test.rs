#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_interface_after_implement() {
    let mut t = TestingT;
    run_test_go_to_definition_interface_after_implement(&mut t);
}

fn run_test_go_to_definition_interface_after_implement(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionInterfaceAfterImplement") {
        return;
    }
    let content = r"interface /*interfaceDefinition*/sInt {
    sVar: number;
    sFn: () => void;
}

class iClass implements /*interfaceReference*/sInt {
    public sVar = 1;
    public sFn() {
    }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["interfaceReference".to_string()]);
    done();
}
