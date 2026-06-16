#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_edits_for_file_rename_tsconfig() {
    let mut t = TestingT;
    run_test_get_edits_for_file_rename_tsconfig(&mut t);
}

fn run_test_get_edits_for_file_rename_tsconfig(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @Filename: /src/tsconfig.json
{
    "compilerOptions": {
        "baseUrl": "./old",
        "paths": {
            "foo": ["old"],
        },
        "rootDir": "old",
        "rootDirs": ["old"],
        "typeRoots": ["old"],
    },
    "files": ["old/a.ts"],
    "include": ["old/*.ts"],
    "exclude": ["old"],
}
// @Filename: /src/old/someFile.ts
"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_will_rename_files_edits(
        t,
        "/src/old",
        "/src/new",
        std::collections::HashMap::from([(
            "/src/tsconfig.json".to_string(),
            r#"{
    "compilerOptions": {
        "baseUrl": "new",
        "paths": {
            "foo": ["new"],
        },
        "rootDir": "new",
        "rootDirs": ["new"],
        "typeRoots": ["new"],
    },
    "files": ["new/a.ts"],
    "include": ["new/*.ts"],
    "exclude": ["new"],
}"#
            .to_string(),
        )]),
    );
    done();
}
