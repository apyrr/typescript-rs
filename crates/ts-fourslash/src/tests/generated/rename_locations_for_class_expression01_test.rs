#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_locations_for_class_expression01() {
    let mut t = TestingT;
    run_test_rename_locations_for_class_expression01(&mut t);
}

fn run_test_rename_locations_for_class_expression01(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameLocationsForClassExpression01") {
        return;
    }
    let content = r#"class Foo {
}

var x = [|class [|{| "contextRangeIndex": 0 |}Foo|] {
    doIt() {
        return [|Foo|];
    }

    static doItStatically() {
        return [|Foo|].y;
    }
}|]

var y = class {
   getSomeName() {
      return Foo
   }
}
var z = class Foo {}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "Foo");
    done();
}
