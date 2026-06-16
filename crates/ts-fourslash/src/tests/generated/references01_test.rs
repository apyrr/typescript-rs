#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references01() {
    let mut t = TestingT;
    run_test_references01(&mut t);
}

fn run_test_references01(t: &mut TestingT) {
    if should_skip_if_failing("TestReferences01") {
        return;
    }
    let content = r#"// @lib: es5
// @Filename: /home/src/workspaces/project/referencesForGlobals_1.ts
class /*0*/globalClass {
    public f() { }
}
// @Filename: /home/src/workspaces/project/referencesForGlobals_2.ts
///<reference path="referencesForGlobals_1.ts" />
var c = /*1*/globalClass();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
