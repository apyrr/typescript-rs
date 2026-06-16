#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_type_param_instantiate_error() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_type_param_instantiate_error(&mut t);
}

fn run_test_code_fix_class_implement_interface_type_param_instantiate_error(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixClassImplementInterfaceTypeParamInstantiateError") {
        return;
    }
    let content = r"interface I<T extends string> {
   x: T;
}

class C implements I<number> { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_available(
        t,
        Some(&vec!["Implement interface 'I<number>'".to_string()]),
    );
    done();
}
