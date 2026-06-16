#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_outlining_spans_switch_cases() {
    let mut t = TestingT;
    run_test_outlining_spans_switch_cases(&mut t);
}

fn run_test_outlining_spans_switch_cases(t: &mut TestingT) {
    if should_skip_if_failing("TestOutliningSpansSwitchCases") {
        return;
    }
    let content = r"switch (undefined)[| {
 case 0:[|
   console.log(1)
   console.log(2)
   break;
   console.log(3);|]
 case 1:[|
   break;|]
 case 2:[|
   break;
   console.log(3);|]
 case 3:[|
   console.log(4);|]
 
 case 4:
 case 5:
 case 6:[|


   console.log(5);|]
 
 case 7:[| console.log(6);|]

 case 8:[| [|{
   console.log(8);
   break;
 }|]
 console.log(8);|]

 default:[|
   console.log(7);
   console.log(8);|]
}|]";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_outlining_spans_from_ranges(t);
    done();
}
