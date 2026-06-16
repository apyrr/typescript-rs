#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_new_keyword01() {
    let mut t = TestingT;
    run_test_quick_info_on_new_keyword01(&mut t);
}

fn run_test_quick_info_on_new_keyword01(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"class Cat {
  /**
   * NOTE: this constructor is private! Please use the factory function
   */
  private constructor() { }

  static makeCat() { new Cat(); }
}

ne/*1*/w Ca/*2*/t();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "1",
        "constructor Cat(): Cat",
        "NOTE: this constructor is private! Please use the factory function",
    );
    f.verify_quick_info_at(
        t,
        "2",
        "constructor Cat(): Cat",
        "NOTE: this constructor is private! Please use the factory function",
    );
    done();
}
