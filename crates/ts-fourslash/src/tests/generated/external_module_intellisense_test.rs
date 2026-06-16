#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_external_module_intellisense() {
    let mut t = TestingT;
    run_test_external_module_intellisense(&mut t);
}

fn run_test_external_module_intellisense(t: &mut TestingT) {
    if should_skip_if_failing("TestExternalModuleIntellisense") {
        return;
    }
    let content = r"// @module: commonjs
// @Filename: externalModuleIntellisense_file0.ts
export = express;
function express(): express.ExpressServer;
namespace express {
    export interface ExpressServer {
        enable(name: string): ExpressServer;
        post(path: RegExp, handler: (req: Function) => void): void;
    }
    export class ExpressServerRequest {
    }
}
// @Filename: externalModuleIntellisense_file1.ts
///<reference path='externalModuleIntellisense_file0.ts'/>
import express = require('./externalModuleIntellisense_file0');
var x = express();/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_number_of_errors_in_current_file(0);
    f.go_to_eof(t);
    f.insert(t, "x.");
    f.verify_completions(
        t,
        MarkerInput::None,
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: Vec::new(),
                excludes: Vec::new(),
                exact: vec![
                    CompletionsExpectedItem::Label("enable".to_string()),
                    CompletionsExpectedItem::Label("post".to_string()),
                ],
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
