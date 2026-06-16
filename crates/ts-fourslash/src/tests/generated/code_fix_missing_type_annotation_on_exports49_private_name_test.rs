#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports49_private_name() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports49_private_name(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports49_private_name(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixMissingTypeAnnotationOnExports49-private-name") {
        return;
    }
    let content = r#"// @isolatedDeclarations: true
// @declaration: true
// @moduleResolution: bundler
// @target: es2018
// @jsx: react-jsx
export function two() {
    const y = "";
    return {} as typeof y;
}

export function three() {
    type Z = string;
    return {} as Z;
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add return type '\"\"'".to_string(),
            new_file_content: r#"export function two(): "" {
    const y = "";
    return {} as typeof y;
}

export function three() {
    type Z = string;
    return {} as Z;
}"#
            .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add return type 'string'".to_string(),
            new_file_content: r#"export function two() {
    const y = "";
    return {} as typeof y;
}

export function three(): string {
    type Z = string;
    return {} as Z;
}"#
            .to_string(),
            new_range_content: String::new(),
            index: 1,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
