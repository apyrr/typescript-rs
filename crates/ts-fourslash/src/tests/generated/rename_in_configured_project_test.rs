#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_in_configured_project() {
    let mut t = TestingT;
    run_test_rename_in_configured_project(&mut t);
}

fn run_test_rename_in_configured_project(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: referencesForGlobals_1.ts
[|var [|{| "contextRangeIndex": 0 |}globalName|] = 0;|]
// @Filename: referencesForGlobals_2.ts
var y = [|globalName|];
// @Filename: tsconfig.json
{ "files": ["referencesForGlobals_1.ts", "referencesForGlobals_2.ts"], "compilerOptions": { "lib": ["es5"] } }"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        f.ranges()[1..].iter().cloned().map(Into::into).collect(),
    );
    done();
}
