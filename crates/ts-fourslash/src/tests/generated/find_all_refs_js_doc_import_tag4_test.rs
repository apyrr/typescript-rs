#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_js_doc_import_tag4() {
    let mut t = TestingT;
    run_test_find_all_refs_js_doc_import_tag4(&mut t);
}

fn run_test_find_all_refs_js_doc_import_tag4(t: &mut TestingT) {
    if should_skip_if_failing("TestFindAllRefsJsDocImportTag4") {
        return;
    }
    let content = r"// @checkJs: true
// @Filename: /component.js
export class Component {
  constructor() {
    this.id_ = Math.random();
  }
  id() {
    return this.id_;
  }
}
// @Filename: /spatial-navigation.js
/** @import * as C from './component.js' */

export class SpatialNavigation {
  /**
   * @param {C.Component} component
   */
  add(component) {}
}
// @Filename: /player.js
import * as C from './component.js';

/**
 * @extends C/*1*/.Component
 */
export class Player extends Component {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
