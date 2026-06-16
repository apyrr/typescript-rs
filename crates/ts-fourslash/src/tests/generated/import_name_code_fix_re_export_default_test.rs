#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_re_export_default() {
    let mut t = TestingT;
    run_test_import_name_code_fix_re_export_default(&mut t);
}

fn run_test_import_name_code_fix_re_export_default(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /user.ts
foo;
// @Filename: /user2.ts
unnamed;
// @Filename: /user3.ts
reExportUnnamed;
// @Filename: /reExportNamed.ts
export { default } from "./named";
// @Filename: /reExportUnnamed.ts
export { default } from "./unnamed";
// @Filename: /named.ts
function foo() {}
export default foo;
// @Filename: /unnamed.ts
export default 0;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/user.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import foo from "./named";

foo;"#
                .to_string(),
            r#"import foo from "./reExportNamed";

foo;"#
                .to_string(),
        ],
        None,
    );
    f.go_to_file(t, "/user2.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import unnamed from "./unnamed";

unnamed;"#
                .to_string(),
            r#"import unnamed from "./reExportUnnamed";

unnamed;"#
                .to_string(),
        ],
        None,
    );
    f.go_to_file(t, "/user3.ts");
    f.verify_import_fix_at_position(
        t,
        &vec![
            r#"import reExportUnnamed from "./reExportUnnamed";

reExportUnnamed;"#
                .to_string(),
        ],
        None,
    );
    done();
}
