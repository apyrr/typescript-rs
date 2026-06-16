#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_code_fix_missing_type_annotation_on_exports23_heritage_formatting() {
    let mut t = TestingT;
    run_test_code_fix_missing_type_annotation_on_exports23_heritage_formatting(&mut t);
}

fn run_test_code_fix_missing_type_annotation_on_exports23_heritage_formatting(t: &mut TestingT) {
    if should_skip_if_failing("TestCodeFixMissingTypeAnnotationOnExports23-heritage-formatting") {
        return;
    }
    let content = r"// @isolatedDeclarations: true
// @declaration: true
function mixin<T extends new (...a: any) => any>(ctor: T): T {
    return ctor;
}
class Point2D { x = 0; y = 0; }
interface I{}
export class Point3D extends
    /** Base class */
    mixin(Point2D)
    // Test
    implements I
    {
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
interface I{}
const Point3DBase: typeof Point2D =
    /** Base class */
    mixin(Point2D);
export class Point3D extends Point3DBase
    // Test
    implements I
    {
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
