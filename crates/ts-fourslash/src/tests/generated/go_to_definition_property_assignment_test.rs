#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_property_assignment() {
    let mut t = TestingT;
    run_test_go_to_definition_property_assignment(&mut t);
}

fn run_test_go_to_definition_property_assignment(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"export const /*FunctionResult*/Component = () => { return "OK"}
Component./*PropertyResult*/displayName = 'Component'

[|/*FunctionClick*/Component|]

Component.[|/*PropertyClick*/displayName|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &["FunctionClick".to_string(), "PropertyClick".to_string()],
    );
    done();
}
