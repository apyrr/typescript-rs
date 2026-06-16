#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_implementation_type_alias_00() {
    let mut t = TestingT;
    run_test_go_to_implementation_type_alias_00(&mut t);
}

fn run_test_go_to_implementation_type_alias_00(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: def.d.ts
export type TypeAlias = { P: number }
// @Filename: ref.ts
import { TypeAlias } from "./def";
const c: T/*ref*/ypeAlias = [|{ P: 2 }|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_implementation(t, &["ref".to_string()]);
    done();
}
