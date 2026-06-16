#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports_type10() {
    let mut t = TestingT;
    run_test_organize_imports_type10(&mut t);
}

fn run_test_organize_imports_type10(t: &mut TestingT) {
    if should_skip_if_failing("TestOrganizeImportsType10") {
        return;
    }
    let content = r#"import {
    type Type1,
    type Type2,
    func4,
    type Type3,
    type Type4,
    type Type5,
    type Type7,
    type Type8,
    type Type9,
    func1,
    func2,
    type Type6,
    func3,
    func5,
    func6,
    func7,
    func8,
    func9,
} from "foo";
interface Use extends Type1, Type2, Type3, Type4, Type5, Type6, Type7, Type8, Type9 {}
console.log(func1, func2, func3, func4, func5, func6, func7, func8, func9);"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_organize_imports(
        t,
        r#"import {
    type Type1,
    type Type2,
    type Type3,
    type Type4,
    type Type5,
    type Type6,
    type Type7,
    type Type8,
    type Type9,
    func1,
    func2,
    func3,
    func4,
    func5,
    func6,
    func7,
    func8,
    func9,
} from "foo";
interface Use extends Type1, Type2, Type3, Type4, Type5, Type6, Type7, Type8, Type9 {}
console.log(func1, func2, func3, func4, func5, func6, func7, func8, func9);"#,
        "source.organizeImports",
        Some(UserPreferences {
            organize_imports_ignore_case: core::TSTrue,
            ..Default::default()
        }),
    );
    done();
}
