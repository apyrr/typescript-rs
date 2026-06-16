#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_deep_inheritance() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_deep_inheritance(&mut t);
}

fn run_test_code_fix_class_implement_deep_inheritance(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @stableTypeOrdering: true
// @strict: false
// Referenced throughout the inheritance chain.
interface I0 { a: number }

class C1 implements I0 { a: number }
interface I1 { b: number }
interface I2 extends C1, I1 {}

class C2 { c: number }
interface I3 {d: number}
class C3 extends C2 implements I0, I2, I3 {
    a: number;
    b: number;
    d: number;
}

interface I4 { e: number }
interface I5 { f: number }
class C4 extends C3 implements I0, I4, I5 {
    e: number;
    f: number;
}

interface I6 extends C4 {}
class C5 implements I6 {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'I6'".to_string(),
            new_file_content: r"// Referenced throughout the inheritance chain.
interface I0 { a: number }

class C1 implements I0 { a: number }
interface I1 { b: number }
interface I2 extends C1, I1 {}

class C2 { c: number }
interface I3 {d: number}
class C3 extends C2 implements I0, I2, I3 {
    a: number;
    b: number;
    d: number;
}

interface I4 { e: number }
interface I5 { f: number }
class C4 extends C3 implements I0, I4, I5 {
    e: number;
    f: number;
}

interface I6 extends C4 {}
class C5 implements I6 {
    c: number;
    a: number;
    b: number;
    d: number;
    e: number;
    f: number;
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
