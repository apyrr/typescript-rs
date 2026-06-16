#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_reference_to_class() {
    let mut t = TestingT;
    run_test_reference_to_class(&mut t);
}

fn run_test_reference_to_class(t: &mut TestingT) {
    if should_skip_if_failing("TestReferenceToClass") {
        return;
    }
    let content = r"// @Filename: referenceToClass_1.ts
class /*1*/foo {
    public n: /*2*/foo;
    public foo: number;
}

class bar {
    public n: /*3*/foo;
    public k = new /*4*/foo();
}

namespace mod {
    var k: /*5*/foo = null;
}
// @Filename: referenceToClass_2.ts
var k: /*6*/foo;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
        ],
    );
    done();
}
