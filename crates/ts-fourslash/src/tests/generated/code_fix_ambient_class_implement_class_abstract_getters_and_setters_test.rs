#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_ambient_class_implement_class_abstract_getters_and_setters() {
    let mut t = TestingT;
    run_test_code_fix_ambient_class_implement_class_abstract_getters_and_setters(&mut t);
}

fn run_test_code_fix_ambient_class_implement_class_abstract_getters_and_setters(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixAmbientClassImplementClassAbstractGettersAndSetters") {
        return;
    }
    let content = r"abstract class A {
    abstract get a(): string;
    abstract set a(newName: string);

    abstract get b(): number;

    abstract set c(arg: number | string);
}

declare class C implements A {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'A'".to_string(),
            new_file_content: r"abstract class A {
    abstract get a(): string;
    abstract set a(newName: string);

    abstract get b(): number;

    abstract set c(arg: number | string);
}

declare class C implements A {
    get a(): string;
    set a(newName: string);
    get b(): number;
    set c(arg: string | number);
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
