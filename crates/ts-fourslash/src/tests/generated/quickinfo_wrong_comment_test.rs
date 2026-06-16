#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quickinfo_wrong_comment() {
    let mut t = TestingT;
    run_test_quickinfo_wrong_comment(&mut t);
}

fn run_test_quickinfo_wrong_comment(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickinfoWrongComment") {
        return;
    }
    let content = r#"// @stableTypeOrdering: true
// @lib: es5
interface I {
    /** The colour */
    readonly colour: string
}
interface A extends I {
    readonly colour: "red" | "green";
}
interface B extends I {
    readonly colour: "yellow" | "green";
}
type F = A | B
const f: F = { colour: "green" }
f.colour/*1*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.go_to_marker(t, "1");
    f.verify_quick_info_is(
        t,
        "(property) colour: \"green\" | \"red\" | \"yellow\"",
        "The colour",
    );
    done();
}
