#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports13() {
    let mut t = TestingT;
    run_test_organize_imports13(&mut t);
}

fn run_test_organize_imports13(t: &mut TestingT) {
    if should_skip_if_failing("TestOrganizeImports13") {
        return;
    }
    let content = r#"import {
    Type1,
    Type2,
    func4,
    Type3,
    Type4,
    Type5,
    Type7,
    Type8,
    Type9,
    func1,
    func2,
    Type6,
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
    Type1,
    Type2,
    Type3,
    Type4,
    Type5,
    Type6,
    Type7,
    Type8,
    Type9,
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
        None,
    );
    f.verify_organize_imports(
        t,
        r#"import {
    func1,
    func2,
    func3,
    func4,
    func5,
    func6,
    func7,
    func8,
    func9,
    Type1,
    Type2,
    Type3,
    Type4,
    Type5,
    Type6,
    Type7,
    Type8,
    Type9,
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
