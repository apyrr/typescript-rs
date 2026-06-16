#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_for_default_export() {
    let mut t = TestingT;
    run_test_find_all_refs_for_default_export(&mut t);
}

fn run_test_find_all_refs_for_default_export(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: a.ts
export default function /*def*/f() {}
// @Filename: b.ts
import /*deg*/g from "./a";
[|/*ref*/g|]();
// @Filename: c.ts
import { f } from "./a";"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["def".to_string(), "deg".to_string()]);
    f.verify_baseline_go_to_definition(t, &["ref".to_string()]);
    done();
}
