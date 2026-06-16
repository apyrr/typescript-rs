#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_quote_preference_double() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_quote_preference_double(&mut t);
}

fn run_test_code_fix_class_implement_interface_quote_preference_double(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixClassImplementInterface_quotePreferenceDouble") {
        return;
    }
    let content = r#"interface I {
    a(): void;
    b(x: "x", y: "a" | "b"): "b";

    c: "c";
    d: { e: "e"; };
}
class Foo implements I {}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'I'".to_string(),
            new_file_content: r#"interface I {
    a(): void;
    b(x: "x", y: "a" | "b"): "b";

    c: "c";
    d: { e: "e"; };
}
class Foo implements I {
    a(): void {
        throw new Error("Method not implemented.");
    }
    b(x: "x", y: "a" | "b"): "b" {
        throw new Error("Method not implemented.");
    }
    c: "c";
    d: { e: "e"; };
}"#
            .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: Some(UserPreferences {
                quote_preference: lsutil::QuotePreference::Double,
                ..Default::default()
            }),
        },
    );
    done();
}
