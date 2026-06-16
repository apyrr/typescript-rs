#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_go_to_definition_class_in_different_file() {
    let mut t = TestingT;
    run_test_tsx_go_to_definition_class_in_different_file(&mut t);
}

fn run_test_tsx_go_to_definition_class_in_different_file(t: &mut TestingT) {
    if should_skip_if_failing("TestTsxGoToDefinitionClassInDifferentFile") {
        return;
    }
    let content = r#"// @jsx: preserve
// @Filename: C.tsx
export default class /*def*/C {}
// @Filename: a.tsx
import C from "./C";
const foo = </*use*/C />;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_go_to_definition(t, &["use".to_string()]);
    done();
}
