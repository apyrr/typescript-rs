#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_property_signatures() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_property_signatures(&mut t);
}

fn run_test_code_fix_class_implement_interface_property_signatures(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface I {
    a0: {};
    a1: { (b1: number, c1: string): number; };
    a2: (b2: number, c2: string) => number;
    a3: { (b3: number, c3: string): number, x: number };

    a4: { new (b1: number, c1: string): number; };
    a5: new (b2: number, c2: string) => number;
    a6: { new (b3: number, c3: string): number, x: number };

    a7: { foo(b7: number, c7: string): number };

    a8: { (b81: number, c81: string): number, new (b82: number, c82: string): number; };

    a9: { (b9: number, c9: string): number; [d9: number]: I };
    a10: { (b10: number, c10: string): number; [d10: string]: I };
}
class C implements I {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'I'".to_string(),
            new_file_content: r"interface I {
    a0: {};
    a1: { (b1: number, c1: string): number; };
    a2: (b2: number, c2: string) => number;
    a3: { (b3: number, c3: string): number, x: number };

    a4: { new (b1: number, c1: string): number; };
    a5: new (b2: number, c2: string) => number;
    a6: { new (b3: number, c3: string): number, x: number };

    a7: { foo(b7: number, c7: string): number };

    a8: { (b81: number, c81: string): number, new (b82: number, c82: string): number; };

    a9: { (b9: number, c9: string): number; [d9: number]: I };
    a10: { (b10: number, c10: string): number; [d10: string]: I };
}
class C implements I {
    a0: {};
    a1: (b1: number, c1: string) => number;
    a2: (b2: number, c2: string) => number;
    a3: { (b3: number, c3: string): number; x: number; };
    a4: new (b1: number, c1: string) => number;
    a5: new (b2: number, c2: string) => number;
    a6: { new(b3: number, c3: string): number; x: number; };
    a7: { foo(b7: number, c7: string): number; };
    a8: { (b81: number, c81: string): number; new(b82: number, c82: string): number; };
    a9: { (b9: number, c9: string): number;[d9: number]: I; };
    a10: { (b10: number, c10: string): number;[d10: string]: I; };
}"
            .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
