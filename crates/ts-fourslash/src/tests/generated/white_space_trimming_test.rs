#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_white_space_trimming() {
    let mut t = TestingT;
    run_test_white_space_trimming(&mut t);
}

fn run_test_white_space_trimming(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"if (true) {     
  //    
   /*err*/}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "err");
    f.insert(t, "\n");
    f.verify_current_file_content(
        t,
        r"if (true) {     
  //    

}",
    );
    done();
}
