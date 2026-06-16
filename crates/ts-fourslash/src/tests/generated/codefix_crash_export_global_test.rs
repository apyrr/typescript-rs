#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_codefix_crash_export_global() {
    let mut t = TestingT;
    run_test_codefix_crash_export_global(&mut t);
}

fn run_test_codefix_crash_export_global(t: &mut TestingT) {
    if should_skip_if_failing("TestCodefixCrashExportGlobal") {
        return;
    }
    let content = r"// @module: commonjs
// @esModuleInterop: false
// @allowSyntheticDefaultImports: false
// @Filename: bar.ts
import * as foo from './foo'
export as namespace foo
export = foo;

declare global {
    const foo: typeof foo;
}
// @Filename: foo.d.ts
interface Root {
    /**
     * A .default property for ES6 default import compatibility
     */
    default: Root;
}

declare const root: Root;
export = root;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "bar.ts");
    f.verify_code_fix_not_available(t, &[]);
    f.go_to_file(t, "foo.d.ts");
    f.verify_code_fix_not_available(t, &[]);
    done();
}
