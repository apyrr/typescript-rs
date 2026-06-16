#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_multiple_function_arguments() {
    let mut t = TestingT;
    run_test_format_multiple_function_arguments(&mut t);
}

fn run_test_format_multiple_function_arguments(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"
 someRandomFunction({
   prop1: 1,
   prop2: 2
 }, {
   prop3: 3,
   prop4: 4
 }, {
   prop5: 5,
   prop6: 6
 });

 someRandomFunction(
     { prop7: 1, prop8: 2 },
     { prop9: 3, prop10: 4 },
     {
       prop11: 5,
       prop2: 6
     }
 );";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"
someRandomFunction({
    prop1: 1,
    prop2: 2
}, {
    prop3: 3,
    prop4: 4
}, {
    prop5: 5,
    prop6: 6
});

someRandomFunction(
    { prop7: 1, prop8: 2 },
    { prop9: 3, prop10: 4 },
    {
        prop11: 5,
        prop2: 6
    }
);",
    );
    done();
}
