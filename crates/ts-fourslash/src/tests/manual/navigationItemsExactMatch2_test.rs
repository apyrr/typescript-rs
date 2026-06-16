use crate::{new_fourslash, TestingT, VerifyWorkspaceSymbolCase};
use ts_lsproto as lsproto;

pub fn test_navigation_items_exact_match2(t: &mut TestingT) {
    let content = r#"module Shapes {
    [|class Point {
        [|private _origin = 0.0;|]
        [|private distanceFromA = 0.0;|]

        [|get distance1(distanceParam): number {
            var [|distanceLocal|];
            return 0;
        }|]
    }|]
}

var [|point = new Shapes.Point()|];
[|function distance2(distanceParam1): void {
    var [|distanceLocal1|];
}|]"#;
    let (f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let ranges = f.ranges();
    f.verify_workspace_symbol(&[
        VerifyWorkspaceSymbolCase {
            pattern: "point".to_string(),
            preferences: None,
            includes: None,
            exact: Some(vec![
                symbol_information(
                    "Point",
                    lsproto::SymbolKindClass,
                    ranges[0].ls_location(),
                    Some("Shapes"),
                ),
                symbol_information(
                    "point",
                    lsproto::SymbolKindVariable,
                    ranges[5].ls_location(),
                    None,
                ),
            ]),
        },
        VerifyWorkspaceSymbolCase {
            pattern: "distance".to_string(),
            preferences: None,
            includes: None,
            exact: Some(vec![
                symbol_information(
                    "distance1",
                    lsproto::SymbolKindProperty,
                    ranges[3].ls_location(),
                    Some("Point"),
                ),
                symbol_information(
                    "distance2",
                    lsproto::SymbolKindFunction,
                    ranges[6].ls_location(),
                    None,
                ),
                symbol_information(
                    "distanceFromA",
                    lsproto::SymbolKindProperty,
                    ranges[2].ls_location(),
                    Some("Point"),
                ),
                symbol_information(
                    "distanceLocal",
                    lsproto::SymbolKindVariable,
                    ranges[4].ls_location(),
                    Some("distance1"),
                ),
                symbol_information(
                    "distanceLocal1",
                    lsproto::SymbolKindVariable,
                    ranges[7].ls_location(),
                    Some("distance2"),
                ),
            ]),
        },
        VerifyWorkspaceSymbolCase {
            pattern: "origin".to_string(),
            preferences: None,
            includes: None,
            exact: Some(vec![symbol_information(
                "_origin",
                lsproto::SymbolKindProperty,
                ranges[1].ls_location(),
                Some("Point"),
            )]),
        },
        VerifyWorkspaceSymbolCase {
            pattern: "square".to_string(),
            preferences: None,
            includes: None,
            exact: Some(Vec::new()),
        },
    ]);
    done();
}

fn symbol_information(
    name: &str,
    kind: lsproto::SymbolKind,
    location: lsproto::Location,
    container_name: Option<&str>,
) -> lsproto::SymbolInformation {
    lsproto::SymbolInformation {
        name: name.to_string(),
        kind,
        location,
        container_name: container_name.map(|value| value.to_string()),
    }
}

