use crate::{new_fourslash, TestingT};

pub fn test_completion_resolve_after_edit(t: &mut TestingT) {
    let content = r#"
// @filename: /index.ts
interface Point {
	x: number;
	y: number;
}
declare const p: Point;
/*a*/

// @filename: /foo.ts
/*b*/
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    // Step 1: Get completions at the marker.
    f.go_to_marker(t, "a");
    let completions = f.get_completions(t, None /*userPreferences*/);
    if completions.items.is_empty() {
        panic!("Expected completions but got none");
    }
    let first_item = completions.items[0].clone();

    // Step 2: Make a file change (insert a comment after marker).
    f.go_to_marker(t, "b");
    f.insert(t, "1");

    // Step 3: Resolve the first completion item from the original list.
    let resolved = f.resolve_completion_item(t, first_item);
    if resolved.is_none() {
        panic!("Expected resolved completion item but got nil");
    }
    done();
}

