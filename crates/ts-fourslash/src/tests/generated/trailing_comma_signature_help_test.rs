#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_trailing_comma_signature_help() {
    let mut t = TestingT;
    run_test_trailing_comma_signature_help(&mut t);
}

fn run_test_trailing_comma_signature_help(t: &mut TestingT) {
    if should_skip_if_failing("TestTrailingCommaSignatureHelp") {
        return;
    }
    let content = r#"function str(n: number): string;
/**
 * Stringifies a number with radix
 * @param radix The radix
 */
function str(n: number, radix: number): string;
function str(n: number, radix?: number): string { return ""; }

str(1, /*a*/)

declare function f<T>(a: T): T;
f(2, /*b*/);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_signature_help(t, &[]);
    done();
}
