#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_var_in_arrow_expression() {
    let mut t = TestingT;
    run_test_quick_info_on_var_in_arrow_expression(&mut t);
}

fn run_test_quick_info_on_var_in_arrow_expression(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface IMap<T> {
    [key: string]: T;
}
var map: IMap<string[]>;
var categories: string[];
each(categories, category => {
    var /*1*/changes = map[category];
    return each(changes, change => {
    });
});
function each<T>(items: T[], handler: (item: T) => void) { }";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(local var) changes: string[]", "");
    done();
}
