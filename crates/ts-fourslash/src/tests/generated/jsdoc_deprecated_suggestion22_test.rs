#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_jsdoc_deprecated_suggestion22() {
    let mut t = TestingT;
    run_test_jsdoc_deprecated_suggestion22(&mut t);
}

fn run_test_jsdoc_deprecated_suggestion22(t: &mut TestingT) {
    if should_skip_if_failing("TestJsdocDeprecated_suggestion22") {
        return;
    }
    let content = r"// @filename: /a.ts
const foo: {
    /**
	 * @deprecated
	 */
	(a: string, b: string): string;
	(a: string, b: number): string;
} = (a: string, b: string | number) => a + b;

[|foo|](1, 1);";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_suggestion_diagnostics(&[]);
    done();
}
