use crate::{new_fourslash, TestingT};

pub fn test_go_to_definition_shorthand_object_literal_with_interface(t: &mut TestingT) {
    let content = r#"interface Something {
    [|foo|]: string;
}

function makeSomething([|foo|]: string): Something {
    return { [|f/*1*/oo|] };
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(t, &["1".to_string()]);
    done();
}

