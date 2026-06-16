#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_comments_and_strings4() {
    let mut t = TestingT;
    run_test_rename_comments_and_strings4(&mut t);
}

fn run_test_rename_comments_and_strings4(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"///<reference path="./Bar.ts" />
[|function [|{| "contextRangeIndex": 0 |}Bar|]() {
    // This is a reference to [|Bar|] in a comment.
    "this is a reference to [|Bar|] in a string";
    ` + "`" + `Foo [|Bar|] Baz.` + "`" + `;
    {
        const Bar = 0;
        ` + "`" + `[|Bar|] ba ${Bar} bara [|Bar|] berbobo ${Bar} araura [|Bar|] ara!` + "`" + `;
    }
}|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(t, vec![f.ranges()[1].clone().into()]);
    done();
}
