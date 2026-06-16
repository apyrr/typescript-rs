use super::{Logger, new_log_tree};

#[test]
fn test_log_tree_implements_logger() {
    fn assert_logger<T: Logger>(_logger: &T) {}
    let tree = new_log_tree("test");
    assert_logger(tree.as_ref());
}

#[test]
fn test_log_tree() {
    let tree = new_log_tree("root");
    tree.log(&["first"]);

    let child = tree.fork("forked");
    child.log(&["child"]);

    let embedded = new_log_tree("embedded");
    embedded.log(&["embedded child"]);
    tree.embed(embedded);

    let output = tree.to_string();
    assert!(output.starts_with("======== root ========\n"));
    assert!(output.contains(" first\n"));
    assert!(output.contains(" forked\n"));
    assert!(output.contains("\t"));
    assert!(output.contains(" child\n"));
    assert!(output.contains(" embedded\n"));
    assert!(output.contains(" embedded child\n"));
}

#[test]
#[should_panic(expected = "can only call String on root LogTree")]
fn test_log_tree_string_panics_on_child() {
    let tree = new_log_tree("root");
    let child = tree.fork("child");
    let _ = child.to_string();
}
