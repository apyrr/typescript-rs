#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_add_var_to_constructor1() {
    let mut t = TestingT;
    run_test_add_var_to_constructor1(&mut t);
}

fn run_test_add_var_to_constructor1(t: &mut TestingT) {
    if should_skip_if_failing("TestAddVarToConstructor1") {
        return;
    }
    let content = r"
//_modes. // produces an internal error - please implement in derived class

namespace editor {
 import modes = _modes;
 
 var i : modes.IMode;
  
 // If you just use p1:modes, the compiler accepts it - should be an error
 class Bg {
     constructor(p1: modes, p2: modes.Mode) {// should be an error on p2 - it's not exported
     /*1*/}
    
 }
}
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.disable_formatting();
    f.go_to_marker(t, "1");
    f.insert(t, "         var x:modes.Mode;\n");
    done();
}
