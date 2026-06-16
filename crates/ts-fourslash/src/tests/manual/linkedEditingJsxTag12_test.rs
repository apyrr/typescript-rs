use crate::{new_fourslash, TestingT};
use std::collections::BTreeMap;

pub fn test_linked_editing_jsx_tag12(t: &mut TestingT) {
    let content = r#"// @Filename: /incomplete.tsx
function Test() {
    return <div>
        </*0*/
        <div {...{}}>
        </div>
    </div>
}
// @Filename: /incompleteMismatched.tsx
function Test() {
    return <div>
        <T
        <div {...{}}>
        </div>
    </div>
}
// @Filename: /incompleteMismatched2.tsx
function Test() {
    return <div>
        <T
        <div {...{}}>
        T</div>
    </div>
}
// @Filename: /incompleteMismatched3.tsx
function Test() {
    return <div>
        <div {...{}}>
        </div>
        <T
    </div>
}
// @Filename: /mismatched.tsx
function Test() {
    return <div>
        <T>
        <div {...{}}>
        </div>
    </div>
}
// @Filename: /matched.tsx
function Test() {
    return <div>

        <div {...{}}>
        </div>
    </div>
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_linked_editing_at_markers(t, BTreeMap::from([("0".to_string(), Vec::new())]));
    f.verify_baseline_linked_editing(t);
    done();
}

