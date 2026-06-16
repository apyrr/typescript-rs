#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_list_at_identifier_definition_locations_destructuring() {
    let mut t = TestingT;
    run_test_completion_list_at_identifier_definition_locations_destructuring(&mut t);
}

fn run_test_completion_list_at_identifier_definition_locations_destructuring(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: a.ts
var [x/*variable1*/
// @Filename: b.ts
var [x, y/*variable2*/
// @Filename: c.ts
var [./*variable3*/
// @Filename: d.ts
var [x, ...z/*variable4*/
// @Filename: e.ts
var {x/*variable5*/
// @Filename: f.ts
var {x, y/*variable6*/
// @Filename: g.ts
function func1({ a/*parameter1*/
// @Filename: h.ts
function func2({ a, b/*parameter2*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_completions(t, MarkerInput::Markers(f.markers()), None);
    done();
}
