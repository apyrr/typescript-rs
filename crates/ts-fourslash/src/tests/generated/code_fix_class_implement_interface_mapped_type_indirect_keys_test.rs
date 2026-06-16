#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_class_implement_interface_mapped_type_indirect_keys() {
    let mut t = TestingT;
    run_test_code_fix_class_implement_interface_mapped_type_indirect_keys(&mut t);
}

fn run_test_code_fix_class_implement_interface_mapped_type_indirect_keys(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixClassImplementInterfaceMappedTypeIndirectKeys") {
        return;
    }
    let content = r"type Base = { ax: number; ay: string };
type BaseKeys = keyof Base;
type MappedIndirect = { [K in BaseKeys]: boolean };
class MappedImpl implements MappedIndirect { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Implement interface 'MappedIndirect'".to_string(),
            new_file_content: r"type Base = { ax: number; ay: string };
type BaseKeys = keyof Base;
type MappedIndirect = { [K in BaseKeys]: boolean };
class MappedImpl implements MappedIndirect {
    ax: boolean;
    ay: boolean;
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
