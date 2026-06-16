use crate::{new_fourslash, TestingT};

pub fn test_rename_default_keyword(t: &mut TestingT) {
    let content = r#"
// @noLib: true
function f(value: string, /*1*/default: string) {}

const /*2*/default = 1;

function /*3*/default() {}

class /*4*/default {}

const foo = {
    /*5*/[|default|]: 1
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let markers = ["1", "2", "3", "4"];
    for marker in markers {
        f.verify_rename_failed(t, marker);
    }

    f.go_to_marker(t, "5");
    f.verify_rename_succeeded_at_current_position();
    done();
}

