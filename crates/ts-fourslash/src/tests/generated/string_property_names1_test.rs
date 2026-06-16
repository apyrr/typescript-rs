#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_string_property_names1() {
    let mut t = TestingT;
    run_test_string_property_names1(&mut t);
}

fn run_test_string_property_names1(t: &mut TestingT) {
    if should_skip_if_failing("TestStringPropertyNames1") {
        return;
    }
    let content = r#"export interface Album {
   "artist": number;
}
var a: Album;
var /**/x = a['artist'];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "", "var x: number", "");
    done();
}
