#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_rename1() {
    let mut t = TestingT;
    run_test_tsx_rename1(&mut t);
}

fn run_test_tsx_rename1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"//@Filename: file.tsx
declare namespace JSX {
    interface Element { }
    interface IntrinsicElements {
        [|[|{| "contextRangeIndex": 0 |}div|]: {
            name?: string;
            isOpen?: boolean;
        };|]
        span: { n: string; };
    }
}
var x = [|<[|{| "contextRangeIndex": 2 |}div|] />|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "div");
    done();
}
