use crate::{new_fourslash, TestingT};
use ts_lsproto as lsproto;

pub fn test_tsconfig_computed_property_error(t: &mut TestingT) {
    let content = r#"// @filename: tsconfig.json
{
    [|["oops!" + 42]|]: "true",
    "compilerOptions": { "lib": ["es5"] },
    "files": [
        "nonexistentfile.ts"
    ],
    "compileOnSave": true
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_non_suggestion_lsp_diagnostics(&[lsproto::Diagnostic {
        message: "String literal with double quotes expected.".to_string(),
        code: Some(lsproto::IntegerOrString::Integer(1327)),
        ..lsproto::Diagnostic::default()
    }]);
    done();
}

