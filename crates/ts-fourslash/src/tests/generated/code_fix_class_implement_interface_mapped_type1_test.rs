#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_mapped_type1() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_mapped_type1(&mut t);
}

fn run_test_code_fix_class_implement_interface_mapped_type1(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixClassImplementInterfaceMappedType1") {
        return;
    }
    let content = r"interface I<X> {
    x: { readonly [K in keyof X]: X[K] };
}
class C<Y> implements I<Y> {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'I<Y>'".to_string(),
            new_file_content: r"interface I<X> {
    x: { readonly [K in keyof X]: X[K] };
}
class C<Y> implements I<Y> {
    x: { readonly [K in keyof Y]: Y[K]; };
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
