#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_parameter_properties() {
    let mut t = TestingT;
    run_test_quick_info_on_parameter_properties(&mut t);
}

fn run_test_quick_info_on_parameter_properties(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoOnParameterProperties") {
        return;
    }
    let content = r"interface IFoo {
  /** this is the name of blabla 
   *  - use blabla 
   *  @example blabla
   */
  name?: string;
}

// test1 should work
class Foo implements IFoo {
  //public name: string = '';
  constructor(
    public na/*1*/me: string, // documentation should leech and work ! 
  ) {
  }
}

// test2 work
class Foo2 implements IFoo {
  public na/*2*/me: string = ''; // documentation leeched and work ! 
  constructor(
    //public name: string,
  ) {
  }
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
