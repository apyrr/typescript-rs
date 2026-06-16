#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_display_parts_class_constructor() {
    let mut t = TestingT;
    run_test_quick_info_display_parts_class_constructor(&mut t);
}

fn run_test_quick_info_display_parts_class_constructor(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"class c {
    /*1*/constructor() {
    }
}
var /*2*/cInstance = new /*3*/c();
var /*4*/cVal = /*5*/c;
class cWithOverloads {
    /*6*/constructor(x: string);
    /*7*/constructor(x: number);
    /*8*/constructor(x: any) {
    }
}
var /*9*/cWithOverloadsInstance = new /*10*/cWithOverloads("hello");
var /*11*/cWithOverloadsInstance2 = new /*12*/cWithOverloads(10);
var /*13*/cWithOverloadsVal = /*14*/cWithOverloads;
class cWithMultipleOverloads {
    /*15*/constructor(x: string);
    /*16*/constructor(x: number);
    /*17*/constructor(x: boolean);
    /*18*/constructor(x: any) {
    }
}
var /*19*/cWithMultipleOverloadsInstance = new /*20*/cWithMultipleOverloads("hello");
var /*21*/cWithMultipleOverloadsInstance2 = new /*22*/cWithMultipleOverloads(10);
var /*23*/cWithMultipleOverloadsInstance3 = new /*24*/cWithMultipleOverloads(true);
var /*25*/cWithMultipleOverloadsVal = /*26*/cWithMultipleOverloads;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_hover(t, &[]);
    done();
}
