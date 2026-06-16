use crate::{new_fourslash, TestingT};

pub fn test_call_hierarchy_incoming_calls_object_literal_method_in_expression_computed_property(
    t: &mut TestingT,
) {
    let content = r#"const obj = {
  [1 + 2]: {
    method() {
      return ""./*split*/split(",");
    }
  }
};
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "split");
    f.verify_baseline_call_hierarchy(t);
    done();
}

