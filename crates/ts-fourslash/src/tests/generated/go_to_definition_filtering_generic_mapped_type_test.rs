#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_filtering_generic_mapped_type() {
    let mut t = TestingT;
    run_test_go_to_definition_filtering_generic_mapped_type(&mut t);
}

fn run_test_go_to_definition_filtering_generic_mapped_type(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"const obj = {
  get /*def*/id() {
    return 1;
  },
  name: "test",
};

type Omit2<T, DroppedKeys extends PropertyKey> = {
  [K in keyof T as Exclude<K, DroppedKeys>]: T[K];
};

declare function omit2<O, Mask extends { [K in keyof O]?: true }>(
  obj: O,
  mask: Mask
): Omit2<O, keyof Mask>;

const obj2 = omit2(obj, {
  name: true,
});

obj2.[|/*ref*/id|];"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["ref".to_string()]);
    done();
}
