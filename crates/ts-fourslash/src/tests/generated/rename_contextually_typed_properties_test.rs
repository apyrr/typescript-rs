#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_rename_contextually_typed_properties() {
    let mut t = TestingT;
    run_test_rename_contextually_typed_properties(&mut t);
}

fn run_test_rename_contextually_typed_properties(t: &mut TestingT) {
    if should_skip_if_failing("TestRenameContextuallyTypedProperties") {
        return;
    }
    let content = r#"interface I {
    [|[|{| "contextRangeIndex": 0 |}prop1|]: () => void;|]
    prop2(): void;
}

var o1: I = {
    [|[|{| "contextRangeIndex": 2 |}prop1|]() { }|],
    prop2() { }
};

var o2: I = {
    [|[|{| "contextRangeIndex": 4 |}prop1|]: () => { }|],
    prop2: () => { }
};

var o3: I = {
    [|get [|{| "contextRangeIndex": 6 |}prop1|]() { return () => { }; }|],
    get prop2() { return () => { }; }
};

var o4: I = {
    [|set [|{| "contextRangeIndex": 8 |}prop1|](v) { }|],
    set prop2(v) { }
};

var o5: I = {
    [|"[|{| "contextRangeIndex": 10 |}prop1|]"() { }|],
    "prop2"() { }
};

var o6: I = {
    [|"[|{| "contextRangeIndex": 12 |}prop1|]": function () { }|],
    "prop2": function () { }
};

var o7: I = {
    [|["[|{| "contextRangeIndex": 14 |}prop1|]"]: function () { }|],
    ["prop2"]: function () { }
};

var o8: I = {
    [|["[|{| "contextRangeIndex": 16 |}prop1|]"]() { }|],
    ["prop2"]() { }
};

var o9: I = {
    [|get ["[|{| "contextRangeIndex": 18 |}prop1|]"]() { return () => { }; }|],
    get ["prop2"]() { return () => { }; }
};

var o10: I = {
    [|set ["[|{| "contextRangeIndex": 20 |}prop1|]"](v) { }|],
    set ["prop2"](v) { }
};"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_rename_at_ranges_with_text(t, "prop1");
    done();
}
