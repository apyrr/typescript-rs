#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_satisfies_expression1() {
    let mut t = TestingT;
    run_test_go_to_definition_satisfies_expression1(&mut t);
}

fn run_test_go_to_definition_satisfies_expression1(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionSatisfiesExpression1") {
        return;
    }
    let content = r"const STRINGS = {
    [|/*definition*/title|]: 'A Title',
} satisfies Record<string,string>;

//somewhere in app
STRINGS.[|/*usage*/title|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["definition".to_string(), "usage".to_string()]);
    done();
}
