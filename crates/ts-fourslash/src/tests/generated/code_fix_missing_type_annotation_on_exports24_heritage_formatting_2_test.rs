#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports24_heritage_formatting_2() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports24_heritage_formatting_2(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports24_heritage_formatting_2(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @isolatedDeclarations: true
// @declaration: true
function mixin<T extends new (...a: any) => any>(ctor: T): T {
    return ctor;
}
class Point2D { x = 0; y = 0; }
export class Point3D2 extends mixin(Point2D) {
    z = 0;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_code_fix_available(t, Some(&vec!["Extract base class to variable".to_string()]));
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Extract base class to variable".to_string(),
            new_file_content: r"function mixin<T extends new (...a: any) => any>(ctor: T): T {
    return ctor;
}
class Point2D { x = 0; y = 0; }
const Point3D2Base: typeof Point2D = mixin(Point2D);
export class Point3D2 extends Point3D2Base {
    z = 0;
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
