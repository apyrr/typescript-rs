#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_illegal_import_clause() {
    let mut t = TestingT;
    run_test_formatting_illegal_import_clause(&mut t);
}

fn run_test_formatting_illegal_import_clause(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"var expect = require('expect.js');
import React   from 'react'/*1*/;
import { mount } from 'enzyme';
require('../setup');
var Amount = require('../../src/js/components/amount');
describe('<Failed />', () => {
  var history
  beforeEach(() => {
    history = createMemoryHistory();
    sinon.spy(history, 'pushState');
  });
  afterEach(() => {
  })
  it('redirects to order summary', () => {
  });
});";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "import React from 'react';");
    done();
}
