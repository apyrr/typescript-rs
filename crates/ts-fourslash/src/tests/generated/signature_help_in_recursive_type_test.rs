#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_in_recursive_type() {
    let mut t = TestingT;
    run_test_signature_help_in_recursive_type(&mut t);
}

fn run_test_signature_help_in_recursive_type(t: &mut TestingT) {
    if should_skip_if_failing("TestSignatureHelpInRecursiveType") {
        return;
    }
    let content = r"type Tail<T extends any[]> =
	((...args: T) => any) extends ((head: any, ...tail: infer R) => any) ? R : never;

type Reverse<List extends any[]> = _Reverse<List, []>;

type _Reverse<Source extends any[], Result extends any[] = []> = {
	1: Result,
	0: _Reverse<Tail<Source>, 0>,
}[Source extends [] ? 1 : 0];

type Foo = Reverse<[0,/**/]>;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("Reverse<List extends any[]>".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 0,
        },
    );
    done();
}
