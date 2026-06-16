#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports27_non_exported_bidings() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports27_non_exported_bidings(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports27_non_exported_bidings(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixMissingTypeAnnotationOnExports27-non-exported-bidings") {
        return;
    }
    let content = r"// @isolatedDeclarations: true
// @declaration: true
let p = { x: 1, y: 2}
const a = 1, b = 10, { x, y } = p, c = 1;
export { x, y }
export const d = a + b + c;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_all(
        t,
        VerifyCodeFixAllOptions {
            fix_id: "fixMissingTypeAnnotationOnExports".to_string(),
            new_file_content: r"let p = { x: 1, y: 2}
const x: number = p.x;
const y: number = p.y;
const a = 1, b = 10, c = 1;
export { x, y }
export const d: number = a + b + c;"
                .to_string(),
        },
    );
    done();
}
