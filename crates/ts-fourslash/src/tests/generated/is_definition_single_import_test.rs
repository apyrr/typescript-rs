#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_is_definition_single_import() {
    let mut t = TestingT;
    run_test_is_definition_single_import(&mut t);
}

fn run_test_is_definition_single_import(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @filename: a.ts
export function /*1*/f() {}
// @filename: b.ts
import { /*2*/f } from "./a";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string()]);
    done();
}
