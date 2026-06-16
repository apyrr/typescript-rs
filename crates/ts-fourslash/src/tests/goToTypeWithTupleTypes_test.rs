use crate::{new_fourslash, TestingT};

pub fn test_go_to_type_with_tuple_types1(t: &mut TestingT) {
    let content = r#"
export let x/*1*/: [number, number] = [1, 2];

type DoubleTupleTrouble<T> = [T, T];

export let y/*2*/: DoubleTupleTrouble<number> = [1, 2];
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let marker_names = f.marker_names();
    f.verify_baseline_go_to_type_definition(t, &marker_names);
    done();
}

