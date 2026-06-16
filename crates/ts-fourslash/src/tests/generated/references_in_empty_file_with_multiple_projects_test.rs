#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_in_empty_file_with_multiple_projects() {
    let mut t = TestingT;
    run_test_references_in_empty_file_with_multiple_projects(&mut t);
}

fn run_test_references_in_empty_file_with_multiple_projects(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesInEmptyFileWithMultipleProjects") {
        return;
    }
    let content = r#"// @Filename: /home/src/workspaces/project/a/tsconfig.json
{ "files": ["a.ts"], "compilerOptions": { "lib": ["es5"] } }
// @Filename: /home/src/workspaces/project/a/a.ts
/// <reference path="../b/b.ts" />
/*1*/;
// @Filename: /home/src/workspaces/project/b/tsconfig.json
{ "files": ["b.ts"], "compilerOptions": { "lib": ["es5"] } }
// @Filename: /home/src/workspaces/project/b/b.ts
/*2*/;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_find_all_references(t, &["1".to_string(), "2".to_string()]);
    done();
}
