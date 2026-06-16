#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_untyped_module() {
    let mut t = TestingT;
    run_test_go_to_definition_untyped_module(&mut t);
}

fn run_test_go_to_definition_untyped_module(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /node_modules/foo/index.js
not read
// @Filename: /a.ts
import { /*def*/f } from "foo";
[|/*use*/f|]();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["use".to_string()]);
    done();
}
