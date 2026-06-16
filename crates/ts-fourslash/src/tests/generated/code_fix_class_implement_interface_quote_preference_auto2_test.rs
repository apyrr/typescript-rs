#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_quote_preference_auto2() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_quote_preference_auto2(&mut t);
}

fn run_test_code_fix_class_implement_interface_quote_preference_auto2(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixClassImplementInterface_quotePreferenceAuto2") {
        return;
    }
    let content = r"// @filename: a.ts
export interface I {
    a(): void;
    b(x: 'x', y: 'a' | 'b'): 'b';

    c: 'c';
    d: { e: 'e'; };
}
// @filename: b.ts
import { I } from './a';
class Foo implements I {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "b.ts");
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'I'".to_string(),
            new_file_content: r"import { I } from './a';
class Foo implements I {
    a(): void {
        throw new Error('Method not implemented.');
    }
    b(x: 'x', y: 'a' | 'b'): 'b' {
        throw new Error('Method not implemented.');
    }
    c: 'c';
    d: { e: 'e'; };
}"
            .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: Some(UserPreferences {
                quote_preference: lsutil::QuotePreference::Auto,
                ..Default::default()
            }),
        },
    );
    done();
}
