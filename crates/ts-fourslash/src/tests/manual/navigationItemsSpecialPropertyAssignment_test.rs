use crate::{new_fourslash, TestingT, VerifyWorkspaceSymbolCase};
use ts_lsproto as lsproto;

pub fn test_navigation_items_special_property_assignment(t: &mut TestingT) {
    let content = r#"// @noLib: true
// @allowJs: true
// @Filename: /a.js
[|exports.x = 0|];
[|exports.z = function() {}|];
function Cls() {
    [|this.instanceProp = 0|];
}
[|Cls.staticMethod = function() {}|];
[|Cls.staticProperty = 0|];
[|Cls.prototype.instanceMethod = function() {}|];"#;
    let (f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    let ranges = f.ranges();
    f.verify_workspace_symbol(&[
        VerifyWorkspaceSymbolCase {
            pattern: "x".to_string(),
            preferences: None,
            includes: None,
            exact: Some(vec![symbol_information(
                "x",
                lsproto::SymbolKindVariable,
                ranges[0].ls_location(),
                None,
            )]),
        },
        VerifyWorkspaceSymbolCase {
            pattern: "z".to_string(),
            preferences: None,
            includes: None,
            exact: Some(vec![symbol_information(
                "z",
                lsproto::SymbolKindVariable,
                ranges[1].ls_location(),
                None,
            )]),
        },
        VerifyWorkspaceSymbolCase {
            pattern: "instanceProp".to_string(),
            preferences: None,
            includes: None,
            exact: Some(vec![symbol_information(
                "instanceProp",
                lsproto::SymbolKindProperty,
                ranges[2].ls_location(),
                Some("Cls"),
            )]),
        },
        VerifyWorkspaceSymbolCase {
            pattern: "staticMethod".to_string(),
            preferences: None,
            includes: None,
            exact: Some(vec![symbol_information(
                "staticMethod",
                lsproto::SymbolKindProperty,
                ranges[3].ls_location(),
                None,
            )]),
        },
        VerifyWorkspaceSymbolCase {
            pattern: "staticProperty".to_string(),
            preferences: None,
            includes: None,
            exact: Some(vec![symbol_information(
                "staticProperty",
                lsproto::SymbolKindProperty,
                ranges[4].ls_location(),
                None,
            )]),
        },
        VerifyWorkspaceSymbolCase {
            pattern: "instanceMethod".to_string(),
            preferences: None,
            includes: None,
            exact: Some(vec![symbol_information(
                "instanceMethod",
                lsproto::SymbolKindProperty,
                ranges[5].ls_location(),
                None,
            )]),
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

