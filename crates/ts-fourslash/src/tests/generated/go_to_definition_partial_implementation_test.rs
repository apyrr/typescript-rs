#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_partial_implementation() {
    let mut t = TestingT;
    run_test_go_to_definition_partial_implementation(&mut t);
}

fn run_test_go_to_definition_partial_implementation(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionPartialImplementation") {
        return;
    }
    let content = r"// @Filename: goToDefinitionPartialImplementation_1.ts
namespace A {
    export interface /*Part1Definition*/IA {
        y: string;
    }
}
// @Filename: goToDefinitionPartialImplementation_2.ts
namespace A {
    export interface /*Part2Definition*/IA {
        x: number;
    }

    var x: [|/*Part2Use*/IA|];
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["Part2Use".to_string()]);
    done();
}
