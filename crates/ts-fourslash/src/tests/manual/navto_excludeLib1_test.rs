use crate::{new_fourslash, TestingT, UserPreferences, VerifyWorkspaceSymbolCase};
use ts_lsproto as lsproto;

pub fn test_navto_exclude_lib1(t: &mut TestingT) {
    let content = r#"// @filename: /index.ts
import { weirdName as otherName } from "bar";
const [|weirdName: number = 1|];
// @filename: /tsconfig.json
{}
// @filename: /node_modules/bar/index.d.ts
export const [|weirdName: number|];
// @filename: /node_modules/bar/package.json
{}"#;
    let (f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let ranges = f.ranges();
    f.verify_workspace_symbol(&[VerifyWorkspaceSymbolCase {
        pattern: "weirdName".to_string(),
        preferences: Some(UserPreferences {
            exclude_library_symbols_in_nav_to: Some(false),
            ..UserPreferences::default()
        }),
        includes: None,
        exact: Some(vec![
            symbol_information(
                "weirdName",
                lsproto::SymbolKindVariable,
                ranges[0].ls_location(),
            ),
            symbol_information(
                "weirdName",
                lsproto::SymbolKindVariable,
                ranges[1].ls_location(),
            ),
        ]),
    }]);
    f.verify_workspace_symbol(&[VerifyWorkspaceSymbolCase {
        pattern: "weirdName".to_string(),
        preferences: None,
        includes: None,
        exact: Some(vec![symbol_information(
            "weirdName",
            lsproto::SymbolKindVariable,
            ranges[0].ls_location(),
        )]),
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

