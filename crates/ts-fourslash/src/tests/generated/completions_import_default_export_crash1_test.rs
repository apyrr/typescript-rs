#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completions_import_default_export_crash1() {
    let mut t = TestingT;
    run_test_completions_import_default_export_crash1(&mut t);
}

fn run_test_completions_import_default_export_crash1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @module: node18
// @allowJs: true
// @Filename: /node_modules/dom7/index.d.ts
export interface Dom7Array {
  length: number;
  prop(propName: string): any;
}

export interface Dom7 {
  (): Dom7Array;
  fn: any;
}

declare const Dom7: Dom7;

export {
  Dom7 as $,
};
// @Filename: /dom7.js
import * as methods from 'dom7';
Object.keys(methods).forEach((methodName) => {
  if (methodName === '$') return;
  methods.$.fn[methodName] = methods[methodName];
});

export default methods.$;
// @Filename: /swipe-back.js
import $ from './dom7.js';
/*1*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(
        t,
        MarkerInput::Name("1".to_string()),
        Some(&CompletionsExpectedList {
            is_incomplete: false,
            item_defaults: Some(CompletionsExpectedItemDefaults {
                commit_characters: Some(default_commit_characters()),
                edit_range: ExpectedCompletionEditRange::Ignored,
            }),
            items: Some(CompletionsExpectedItems {
                includes: vec![CompletionsExpectedItem::Item(lsproto::CompletionItem {
                    label: "$".to_string(),
                    ..Default::default()
                })],
                excludes: Vec::new(),
                exact: Vec::new(),
                unsorted: Vec::new(),
            }),
            user_preferences: None,
        }),
    );
    done();
}
