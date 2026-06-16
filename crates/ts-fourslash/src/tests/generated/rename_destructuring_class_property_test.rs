#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_destructuring_class_property() {
    let mut t = TestingT;
    run_test_rename_destructuring_class_property(&mut t);
}

fn run_test_rename_destructuring_class_property(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameDestructuringClassProperty") {
        return;
    }
    let content = r#"class A {
    [|[|{| "contextRangeIndex": 0 |}foo|]: string;|]
}
class B {
    syntax1(a: A): void {
        [|let { [|{| "contextRangeIndex": 2 |}foo|] } = a;|]
    }
    syntax2(a: A): void {
        [|let { [|{| "contextRangeIndex": 4 |}foo|]: foo } = a;|]
    }
    syntax11(a: A): void {
        [|let { [|{| "contextRangeIndex": 6 |}foo|] } = a;|]
        [|foo|] = "newString";
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_marker_or_ranges(
        t,
        vec![
            f.ranges()[1].clone().into(),
            f.ranges()[5].clone().into(),
            f.ranges()[3].clone().into(),
            f.ranges()[7].clone().into(),
            f.ranges()[8].clone().into(),
        ],
    );
    done();
}
