#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_signature_help_for_signature_with_unreachable_type() {
    let mut t = TestingT;
    run_test_signature_help_for_signature_with_unreachable_type(&mut t);
}

fn run_test_signature_help_for_signature_with_unreachable_type(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /node_modules/foo/node_modules/bar/index.d.ts
export interface SomeType {
    x?: number;
}
// @Filename: /node_modules/foo/index.d.ts
import { SomeType } from "bar";
export function func<T extends SomeType>(param: T): void;
export function func<T extends SomeType>(param: T, other: T): void;
// @Filename: /usage.ts
import { func } from "foo";
func({/*1*/});"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_signature_help_options(
        t,
        VerifySignatureHelpOptions {
            text: Some("func(param: {}): void".to_string()),
            parameter_name: None,
            parameter_span: None,
            parameter_count: None,
            overloads_count: 2,
        },
    );
    done();
}
