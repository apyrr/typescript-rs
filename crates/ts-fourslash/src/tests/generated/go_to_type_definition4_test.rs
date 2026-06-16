#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_type_definition4() {
    let mut t = TestingT;
    run_test_go_to_type_definition4(&mut t);
}

fn run_test_go_to_type_definition4(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: foo.ts
export type /*def0*/T = string;
export const /*def1*/T = "";
// @Filename: bar.ts
import { T } from "./foo";
let x: [|/*reference*/T|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_type_definition(t, &["reference".to_string()]);
    f.verify_baseline_go_to_definition(t, &["reference".to_string()]);
    done();
}
