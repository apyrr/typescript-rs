#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_across_multiple_projects() {
    let mut t = TestingT;
    run_test_go_to_definition_across_multiple_projects(&mut t);
}

fn run_test_go_to_definition_across_multiple_projects(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionAcrossMultipleProjects") {
        return;
    }
    let content = r#"//@Filename: a.ts
var /*def1*/x: number;
//@Filename: b.ts
var /*def2*/x: number;
//@Filename: c.ts
var /*def3*/x: number;
//@Filename: d.ts
var /*def4*/x: number;
//@Filename: e.ts
/// <reference path="a.ts" />
/// <reference path="b.ts" />
/// <reference path="c.ts" />
/// <reference path="d.ts" />
[|/*use*/x|]++;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["use".to_string()]);
    done();
}
