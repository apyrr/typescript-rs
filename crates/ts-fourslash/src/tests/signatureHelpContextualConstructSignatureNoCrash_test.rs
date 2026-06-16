use crate::{new_fourslash, TestingT};

// Tests that signature help does not panic when the contextual type has only construct signatures
// (no call signatures).
pub fn test_signature_help_contextual_construct_signature_no_crash(t: &mut TestingT) {
    let content = r#"
type Obj = {
    foo: new () => object
}

let obj: Obj = {
    foo(/*constructOnly*/) {}
}
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    // When contextual type only has construct signatures (no call signatures),
    // no signature help should be provided (and no panic should occur).
    f.go_to_marker(t, "constructOnly");
    f.verify_no_signature_help(t);
    done();
}

