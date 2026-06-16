use crate::{new_fourslash, TestingT};

pub fn test_rename_builtin_types(t: &mut TestingT) {
    let content = r#"
const arr: /*1*/Array<number> = [];
const map1: /*2*/Map<string, number> = new Map();
const prom: /*3*/Promise<void> = Promise.resolve();
const str: /*4*/string = "hello";
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    // All of these should fail because they're library/builtin types
    for marker in ["1", "2", "3", "4"] {
        f.go_to_marker(t, marker);
        f.verify_rename_failed_at_current_position();
    }
    done();
}

