#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_ambient_shorthand_goto_definition() {
    let mut t = TestingT;
    run_test_ambient_shorthand_goto_definition(&mut t);
}

fn run_test_ambient_shorthand_goto_definition(t: &mut TestingT) {
    if should_skip_if_failing("TestAmbientShorthandGotoDefinition") {
        return;
    }
    let content = r#"// @Filename: declarations.d.ts
declare module /*module*/"jquery"
// @Filename: user.ts
///<reference path="declarations.d.ts"/>
import [|/*importFoo*/foo|], {bar} from "jquery";
import * as [|/*importBaz*/baz|] from "jquery";
import [|/*importBang*/bang|] = require("jquery");
[|foo/*useFoo*/|]([|bar/*useBar*/|], [|baz/*useBaz*/|], [|bang/*useBang*/|]);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "useFoo", "(alias) module \"jquery\"\nimport foo", "");
    f.verify_quick_info_at(t, "useBar", "(alias) module \"jquery\"\nimport bar", "");
    f.verify_quick_info_at(t, "useBaz", "(alias) module \"jquery\"\nimport baz", "");
    f.verify_quick_info_at(
        t,
        "useBang",
        "(alias) module \"jquery\"\nimport bang = require(\"jquery\")",
        "",
    );
    f.verify_baseline_go_to_definition(
        t,
        &[
            "useFoo".to_string(),
            "importFoo".to_string(),
            "useBar".to_string(),
            "useBaz".to_string(),
            "importBaz".to_string(),
            "useBang".to_string(),
            "importBang".to_string(),
        ],
    );
    done();
}
