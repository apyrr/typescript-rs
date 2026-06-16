use crate::{new_fourslash, TestingT, VerifyWorkspaceSymbolCase};
use ts_lsproto as lsproto;

// TestWorkspaceSymbolMultiProjectNonExistentRef verifies that workspace symbol
// requests work correctly in a multi-project scenario where one project's
// tsconfig has a reference to a non-existent path.
pub fn test_workspace_symbol_multi_project_non_existent_ref(t: &mut TestingT) {
    let content = r#"
// @Filename: /home/src/projects/project-a/tsconfig.json
{
  "compilerOptions": { "composite": true },
  "references": [{ "path": "../project-nonexistent" }]
}

// @Filename: /home/src/projects/project-a/index.ts
export const [|myValueA: number = 1|];

// @Filename: /home/src/projects/project-b/tsconfig.json
{
  "compilerOptions": { "composite": true },
  "references": [{ "path": "../project-a" }]
}

// @Filename: /home/src/projects/project-b/index.ts
export const [|myValueB: string = "hello"|];
"#;
    let (f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());

    // Verify we can find symbols from both projects with a single pattern
    let ranges = f.ranges();
    f.verify_workspace_symbol(&[VerifyWorkspaceSymbolCase {
        pattern: "myValue".to_string(),
        includes: Some(vec![
            symbol_information(
                "myValueA",
                lsproto::SymbolKindVariable,
                ranges[0].ls_location(),
            ),
            symbol_information(
                "myValueB",
                lsproto::SymbolKindVariable,
                ranges[1].ls_location(),
            ),
        ]),
        exact: None,
        preferences: None,
    }]);
    done();
}

fn symbol_information(
    name: &str,
    kind: lsproto::SymbolKind,
    location: lsproto::Location,
) -> lsproto::SymbolInformation {
    lsproto::SymbolInformation {
        name: name.to_string(),
        kind,
        location,
        container_name: None,
    }
}

