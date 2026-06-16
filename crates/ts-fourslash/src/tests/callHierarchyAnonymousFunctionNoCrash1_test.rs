use crate::{new_fourslash, TestingT};

pub fn test_call_hierarchy_anonymous_function_no_crash1(t: &mut TestingT) {
    let content = r#"// @Filename: /main.ts
func/*1*/tion() {}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "1");
    f.verify_baseline_call_hierarchy(t);
    done();
}

