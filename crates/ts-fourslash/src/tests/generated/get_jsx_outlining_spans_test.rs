#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_jsx_outlining_spans() {
    let mut t = TestingT;
    run_test_get_jsx_outlining_spans(&mut t);
}

fn run_test_get_jsx_outlining_spans(t: &mut TestingT) {
    if should_skip_if_failing("TestGetJSXOutliningSpans") {
        return;
    }
    let content = r#"import React, { Component } from 'react';

export class Home extends Component[| {
  render()[| {
    return [|(
    [|<div>
      [|<h1>Hello, world!</h1>|]
      [|<ul>
        [|<li>
          [|<a [|href='https://get.asp.net/'|]>
            ASP.NET Core
          </a>|]
        </li>|]
        [|<li>[|<a [|href='https://facebook.github.io/react/'|]>React</a>|] for client-side code</li>|]
        [|<li>[|<a [|href='http://getbootstrap.com/'|]>Bootstrap</a>|] for layout and styling</li>|]
      </ul>|]
      <div
        [|accesskey="test"
        class="active"
        dir="auto"|] />
      <PageHeader [|title="Log in"
        {...[|{
          item: true,
          xs: 9,
          md: 5
        }|]}|]
      />
      [|<>
          text 
      </>|]
    </div>|]
    )|];
  }|]
}|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_outlining_spans_from_ranges(t);
    done();
}
