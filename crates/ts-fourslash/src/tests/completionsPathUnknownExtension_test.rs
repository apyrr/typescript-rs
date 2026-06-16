use crate::{
    new_fourslash, CompletionsExpectedItem, CompletionsExpectedItemDefaults,
    CompletionsExpectedItems, CompletionsExpectedList, ExpectedCompletionEditRange, MarkerInput,
    TestingT,
};

pub fn test_completions_path_unknown_extension(t: &mut TestingT) {
    let content = r##"// @filename: src/some-file.ruhroh
/* This is just a test file that needs to exist. */

// @filename: package.json
{
    "imports": {
        "#/*": "./src/*"
    }
}

// @filename: src/globals.d.ts
declare module "*.ruhroh";

// @filename: src/a.mts
import "#//*$*/"

// @filename: tsconfig.json
{
    "compilerOptions": {
        "module": "preserve",
        "moduleResolution": "bundler",
        "rootDir": "src"
    },
    "include": ["src"]
}"##;

    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let expected = CompletionsExpectedList {
        is_incomplete: false,
        item_defaults: Some(CompletionsExpectedItemDefaults {
            commit_characters: Some(Vec::new()),
            edit_range: ExpectedCompletionEditRange::Ignored,
        }),
        items: Some(CompletionsExpectedItems {
            includes: vec![CompletionsExpectedItem::Label("some-file.ruhroh".to_string())],
            excludes: Vec::new(),
            exact: Vec::new(),
            unsorted: Vec::new(),
        }),
        user_preferences: None,
    };
    f.verify_completions(t, MarkerInput::Name("$".to_string()), Some(&expected));
    done();
}

