#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_occurrences_is_definition_of_variable() {
    let mut t = TestingT;
    run_test_get_occurrences_is_definition_of_variable(&mut t);
}

fn run_test_get_occurrences_is_definition_of_variable(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"/*1*/var /*2*/x = 0;
var assignmentRightHandSide = /*3*/x;
var assignmentRightHandSide2 = 1 + /*4*/x;

/*5*/x = 1;
/*6*/x = /*7*/x + /*8*/x;

/*9*/x == 1;
/*10*/x <= 1;

var preIncrement = ++/*11*/x;
var postIncrement = /*12*/x++;
var preDecrement = --/*13*/x;
var postDecrement = /*14*/x--;

/*15*/x += 1;
/*16*/x <<= 1;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
            "6".to_string(),
            "7".to_string(),
            "8".to_string(),
            "9".to_string(),
            "10".to_string(),
            "11".to_string(),
            "12".to_string(),
            "13".to_string(),
            "14".to_string(),
            "15".to_string(),
            "16".to_string(),
        ],
    );
    done();
}
