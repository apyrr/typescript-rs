use crate::{new_fourslash, TestingT};

pub fn test_go_to_definition_object_binding_pattern(t: &mut TestingT) {
    let content = r#"
interface SomeType {
    targetProperty: number;
}

function foo(callback: (p: SomeType) => void) {}

foo(({ /*1*/targetProperty }) => {
    /*4*/targetProperty
});

let { /*2*/targetProperty }: SomeType = { /*3*/targetProperty: 42 };

let { /*5*/targetProperty: /*6*/alias_1 }: SomeType = { targetProperty: 42 };

let { x: { /*7*/targetProperty: /*8*/{} } }: { x: SomeType } = { x: { targetProperty: 42 } };"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let marker_names = f.marker_names();
    f.verify_baseline_go_to_definition(t, &marker_names);
    done();
}

pub fn test_go_to_definition_object_binding_pattern_rest(t: &mut TestingT) {
    let content = r#"
interface SomeType {
    targetProperty: number;
}

let { .../*1*/rest }: SomeType = { targetProperty: 42 };"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let marker_names = f.marker_names();
    f.verify_baseline_go_to_definition(t, &marker_names);
    done();
}

