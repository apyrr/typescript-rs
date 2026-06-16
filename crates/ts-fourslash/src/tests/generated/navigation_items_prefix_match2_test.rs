#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_navigation_items_prefix_match2() {
    let mut t = TestingT;
    run_test_navigation_items_prefix_match2(&mut t);
}

fn run_test_navigation_items_prefix_match2(t: &mut TestingT) {
    if should_skip_if_failing("TestNavigationItemsPrefixMatch2") {
        return;
    }
    let content = r"// @lib: es5
namespace Shapes {
    export class Point {
        [|private originality = 0.0;|]
        [|private distanceFromOrig = 0.0;|]
        [|get distanceFarFarAway(distanceFarFarAwayParam: number): number {
            var [|distanceFarFarAwayLocal|];
            return 0;
        }|]
    }
}
var pointsSquareBox = new Shapes.Point();
function PointsFunc(): void {
 var pointFuncLocal;
}
[|interface OriginI {
    123;
    [|origin1;|]
    [|public _distance(distanceParam): void;|]
}|]";
    let (f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_workspace_symbol(&[
        workspace_symbol_case(
            "origin",
            vec![
                symbol_information(
                    "origin1",
                    lsproto::SymbolKindProperty,
                    f.ranges()[5].ls_location(),
                    Some("OriginI"),
                ),
                symbol_information(
                    "originality",
                    lsproto::SymbolKindProperty,
                    f.ranges()[0].ls_location(),
                    Some("Point"),
                ),
                symbol_information(
                    "OriginI",
                    lsproto::SymbolKindInterface,
                    f.ranges()[4].ls_location(),
                    None,
                ),
            ],
        ),
        workspace_symbol_case(
            "distance",
            vec![
                symbol_information(
                    "distanceFarFarAway",
                    lsproto::SymbolKindProperty,
                    f.ranges()[2].ls_location(),
                    Some("Point"),
                ),
                symbol_information(
                    "distanceFarFarAwayLocal",
                    lsproto::SymbolKindVariable,
                    f.ranges()[3].ls_location(),
                    Some("distanceFarFarAway"),
                ),
                symbol_information(
                    "distanceFromOrig",
                    lsproto::SymbolKindProperty,
                    f.ranges()[1].ls_location(),
                    Some("Point"),
                ),
                symbol_information(
                    "_distance",
                    lsproto::SymbolKindMethod,
                    f.ranges()[6].ls_location(),
                    Some("OriginI"),
                ),
            ],
        ),
        workspace_symbol_case("mPointThatIJustInitiated wrongKeyWord", vec![]),
    ]);
    done();
}
