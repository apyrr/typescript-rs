#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_function_property_returned_from_generic_function3() {
    let mut t = TestingT;
    run_test_quick_info_on_function_property_returned_from_generic_function3(&mut t);
}

fn run_test_quick_info_on_function_property_returned_from_generic_function3(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"function createProps<T>(t: T) {
  const getProps = () => {}
  const createVariants = () => {}

  getProps.createVariants = createVariants;
  return getProps;
}

createProps({})./**/createVariants();";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(
        t,
        "",
        "(property) getProps<{}>.createVariants: () => void",
        "",
    );
    done();
}
