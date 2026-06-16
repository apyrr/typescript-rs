#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_type_arguments2() {
    let mut t = TestingT;
    run_test_signature_help_type_arguments2(&mut t);
}

fn run_test_signature_help_type_arguments2(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpTypeArguments2") {
        return;
    }
    let content = r"/** some documentation
 * @template T some documentation 2
 * @template W
 * @template U,V others
 * @param a ok
 * @param b not ok
 */
function f<T, U, V, W>(a: number, b: string, c: boolean): void { }
f</*f0*/;
f<number, /*f1*/;
f<number, string, /*f2*/;
f<number, string, boolean, /*f3*/;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_signature_help(t, &[]);
    done();
}
