#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_tsx_rename4() {
    let mut t = TestingT;
    run_test_tsx_rename4(&mut t);
}

fn run_test_tsx_rename4(t: &mut TestingT) {
    if should_skip_if_failing("TestTsxRename4") {
        return;
    }
    let content = r#"// @jsx: preserve
//@Filename: file.tsx
declare namespace JSX {
    interface Element {}
    interface IntrinsicElements {
        div: {};
    }
}
[|class [|{| "contextRangeIndex": 0 |}MyClass|] {}|]

[|<[|{| "contextRangeIndex": 2 |}MyClass|]></[|{| "contextRangeIndex": 2 |}MyClass|]>|];
[|<[|{| "contextRangeIndex": 5 |}MyClass|]/>|];

[|<[|{| "contextRangeIndex": 7 |}div|]> </[|{| "contextRangeIndex": 7 |}div|]>|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_no_errors();
    f.verify_baseline_rename_at_ranges_with_text(t, "MyClass");
    done();
}
