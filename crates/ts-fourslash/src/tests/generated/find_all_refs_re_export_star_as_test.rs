#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_re_export_star_as() {
    let mut t = TestingT;
    run_test_find_all_refs_re_export_star_as(&mut t);
}

fn run_test_find_all_refs_re_export_star_as(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: /leafModule.ts
export const /*helloDef*/hello = () => 'Hello';
// @Filename: /exporting.ts
export * as /*leafDef*/Leaf from './leafModule';
// @Filename: /importing.ts
 import { /*leafImportDef*/Leaf } from './exporting';
 /*leafUse*/[|Leaf|]./*helloUse*/[|hello|]()";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_find_all_references(
        t,
        &[
            "helloDef".to_string(),
            "helloUse".to_string(),
            "leafDef".to_string(),
            "leafImportDef".to_string(),
            "leafUse".to_string(),
        ],
    );
    done();
}
