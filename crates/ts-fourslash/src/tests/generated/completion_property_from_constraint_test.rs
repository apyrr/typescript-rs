#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_completion_property_from_constraint() {
    let mut t = TestingT;
    run_test_completion_property_from_constraint(&mut t);
}

fn run_test_completion_property_from_constraint(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"interface Styles {
  alignContent: string | null;
  alignItems: string | null;
  alignmentBaseline: string | null;
  // etc..
  [key: string]: any
}

interface StyleMap {
  [name: string]: Partial<Styles>
}

declare function createStyles<T extends StyleMap>(styles: T): T

createStyles({
  x: {
    '/*1*/': ''
  }
});";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_completions(t, &[]);
    done();
}
