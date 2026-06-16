#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_untyped_module_import() {
    let mut t = TestingT;
    run_test_quick_info_untyped_module_import(&mut t);
}

fn run_test_quick_info_untyped_module_import(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoUntypedModuleImport") {
        return;
    }
    let content = r#"// @strict: false
// @Filename: node_modules/foo/index.js
 /*index*/{}
// @Filename: a.ts
import /*foo*/foo from /*fooModule*/"foo";
/*fooCall*/foo();"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "a.ts");
    f.verify_number_of_errors_in_current_file(0);
    f.go_to_marker(t, "fooModule");
    f.verify_quick_info_is(t, "", "");
    f.go_to_marker(t, "foo");
    f.verify_quick_info_is(t, "import foo", "");
    f.verify_baseline_find_all_references(
        t,
        &[
            "foo".to_string(),
            "fooModule".to_string(),
            "fooCall".to_string(),
        ],
    );
    f.verify_baseline_go_to_definition(t, &["fooModule".to_string(), "foo".to_string()]);
    done();
}
