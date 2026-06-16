#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_unused_imports14_fs() {
    let mut t = TestingT;
    run_test_unused_imports14_fs(&mut t);
}

fn run_test_unused_imports14_fs(t: &mut TestingT) {
    if should_skip_if_failing("TestUnusedImports14FS") {
        return;
    }
    let content = r"// @noUnusedLocals: true
// @Filename: file2.ts
[| import /* 1 */ A /* 2 */, /* 3 */ { /* 4 */ x /* 5 */ } /* 6 */ from './a'; |]
console.log(A);
// @Filename: file1.ts
export default 10;
export var x = 10;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_range_after_code_fix(
        t,
        "import /* 1 */ A /* 2 */ /* 6 */ from './a';",
        false,
        0,
        0,
    );
    done();
}
