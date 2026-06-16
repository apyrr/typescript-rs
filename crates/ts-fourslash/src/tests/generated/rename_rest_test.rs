#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_rest() {
    let mut t = TestingT;
    run_test_rename_rest(&mut t);
}

fn run_test_rename_rest(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameRest") {
        return;
    }
    let content = r#"interface Gen {
    x: number;
    [|[|{| "contextRangeIndex": 0 |}parent|]: Gen;|]
    millenial: string;
}
let t: Gen;
var { x, ...rest } = t;
rest.[|parent|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "parent");
    done();
}
