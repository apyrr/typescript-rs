#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_edits_for_file_rename_unresolvable_node_module() {
    let mut t = TestingT;
    run_test_get_edits_for_file_rename_unresolvable_node_module(&mut t);
}

fn run_test_get_edits_for_file_rename_unresolvable_node_module(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @allowJs: true
// @checkJs: true
// @Filename: /modules/@app/something/index.js
import "doesnt-exist";
// @Filename: /modules/@local/foo.js
import "doesnt-exist"; "#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_will_rename_files_edits(
        t,
        "/modules/@app/something",
        "/modules/@app/something-2",
        std::collections::HashMap::<String, String>::new(),
    );
    done();
}
