#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_import_name_code_fix_jsx6() {
    let mut t = TestingT;
    run_test_import_name_code_fix_jsx6(&mut t);
}

fn run_test_import_name_code_fix_jsx6(t: &mut TestingT) {
    if should_skip_if_failing("TestImportNameCodeFix_jsx6") {
        return;
    }
    let content = r#"// @lib: es5
// @jsx: react
// @module: esnext
// @esModuleInterop: true
// @moduleResolution: bundler
// @Filename: /node_modules/react/index.d.ts
export = React;
export as namespace React;
declare namespace React {
    class Component {}
}
// @Filename: /node_modules/react-native/index.d.ts
import * as React from "react";
export class Text extends React.Component {};
// @Filename: /a.tsx
<[|Text|]></Text>;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/a.tsx");
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Add import from \"react-native\"".to_string(),
            new_file_content: r#"import { Text } from "react-native";

<Text></Text>;"#
                .to_string(),
            new_range_content: String::new(),
            index: 0,
            apply_changes: false,
            user_preferences: None,
        },
    );
    f.verify_code_fix(
        t,
        VerifyCodeFixOptions {
            description: "Import 'React' from \"react\"".to_string(),
            new_file_content: r#"import React from "react";

<Text></Text>;"#
                .to_string(),
            new_range_content: String::new(),
            index: 1,
            apply_changes: false,
            user_preferences: None,
        },
    );
    done();
}
