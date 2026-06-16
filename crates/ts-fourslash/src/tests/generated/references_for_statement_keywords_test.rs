#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_references_for_statement_keywords() {
    let mut t = TestingT;
    run_test_references_for_statement_keywords(&mut t);
}

fn run_test_references_for_statement_keywords(t: &mut TestingT) {
    if should_skip_if_failing("TestReferencesForStatementKeywords") {
        return;
    }
    let content = r#"// @filename: /main.ts
// import ... = ...
[|{| "id": "importEqualsDecl1" |}/*importEqualsDecl1_importKeyword*/[|import|] [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeId": "importEqualsDecl1" |}A|] = /*importEqualsDecl1_requireKeyword*/[|require|]("[|{| "isWriteAccess": false, "isDefinition": false, "contextRangeId": "importEqualsDecl1" |}./a|]");|]
[|{| "id": "namespaceDecl1" |}namespace [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeId": "namespaceDecl1" |}N|] { }|]
[|{| "id": "importEqualsDecl2" |}/*importEqualsDecl2_importKeyword*/[|import|] [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeId": "importEqualsDecl2" |}N2|] = [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeId": "importEqualsDecl2" |}N|];|]

// import ... from ...
[|{| "id": "importDecl1" |}/*importDecl1_importKeyword*/[|import|] /*importDecl1_typeKeyword*/[|type|] [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeId": "importDecl1" |}B|] /*importDecl1_fromKeyword*/[|from|] "[|{| "isWriteAccess": false, "isDefinition": false, "contextRangeId": "importDecl1" |}./b|]";|]
[|{| "id": "importDecl2" |}/*importDecl2_importKeyword*/[|import|] /*importDecl2_typeKeyword*/[|type|] * /*importDecl2_asKeyword*/[|as|] [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeId": "importDecl2" |}C|] /*importDecl2_fromKeyword*/[|from|] "[|{| "isWriteAccess": false, "isDefinition": false, "contextRangeId": "importDecl2" |}./c|]";|]
[|{| "id": "importDecl3" |}/*importDecl3_importKeyword*/[|import|] /*importDecl3_typeKeyword*/[|type|] { [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeId": "importDecl3" |}D|] } /*importDecl3_fromKeyword*/[|from|] "[|{| "isWriteAccess": false, "isDefinition": false, "contextRangeId": "importDecl3" |}./d|]";|]
[|{| "id": "importDecl4" |}/*importDecl4_importKeyword*/[|import|] /*importDecl4_typeKeyword*/[|type|] { e1, e2 /*importDecl4_asKeyword*/[|as|] [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeId": "importDecl4" |}e3|] } /*importDecl4_fromKeyword*/[|from|] "[|{| "isWriteAccess": false, "isDefinition": false, "contextRangeId": "importDecl4" |}./e|]";|]

// import "module"
[|{| "id": "importDecl5" |}/*importDecl5_importKeyword*/[|import|] "[|{| "isWriteAccess": false, "isDefinition": false, "contextRangeId": "importDecl5" |}./f|]";|]

// export ... from ...
[|{| "id": "exportDecl1" |}/*exportDecl1_exportKeyword*/[|export|] /*exportDecl1_typeKeyword*/[|type|] * /*exportDecl1_fromKeyword*/[|from|] "[|{| "isWriteAccess": false, "isDefinition": false, "contextRangeId": "exportDecl1" |}./g|]";|]
[|{| "id": "exportDecl2" |}/*exportDecl2_exportKeyword*/[|export|] /*exportDecl2_typeKeyword*/[|type|] [|{| "id": "exportDecl2_namespaceExport" |}* /*exportDecl2_asKeyword*/[|as|] [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeId": "exportDecl2" |}H|]|] /*exportDecl2_fromKeyword*/[|from|] "[|{| "isWriteAccess": false, "isDefinition": false, "contextRangeId": "exportDecl2" |}./h|]";|]
[|{| "id": "exportDecl3" |}/*exportDecl3_exportKeyword*/[|export|] /*exportDecl3_typeKeyword*/[|type|] { [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeId": "exportDecl3" |}I|] } /*exportDecl3_fromKeyword*/[|from|] "[|{| "isWriteAccess": false, "isDefinition": false, "contextRangeId": "exportDecl3" |}./i|]";|]
[|{| "id": "exportDecl4" |}/*exportDecl4_exportKeyword*/[|export|] /*exportDecl4_typeKeyword*/[|type|] { j1, j2 /*exportDecl4_asKeyword*/[|as|] [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeId": "exportDecl4" |}j3|] } /*exportDecl4_fromKeyword*/[|from|] "[|{| "isWriteAccess": false, "isDefinition": false, "contextRangeId": "exportDecl4" |}./j|]";|]
[|{| "id": "typeDecl1" |}type [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeId": "typeDecl1" |}Z1|] = 1;|]
[|{| "id": "exportDecl5" |}/*exportDecl5_exportKeyword*/[|export|] /*exportDecl5_typeKeyword*/[|type|] { [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeId": "exportDecl5" |}Z1|] };|]
type Z2 = 2;
type Z3 = 3;
[|{| "id": "exportDecl6" |}/*exportDecl6_exportKeyword*/[|export|] /*exportDecl6_typeKeyword*/[|type|] { z2, z3 /*exportDecl6_asKeyword*/[|as|] [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeId": "exportDecl6" |}z4|] };|]
// @filename: /main2.ts
[|{| "id": "varDecl1" |}const [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeId": "varDecl1" |}x|] = {};|]
[|{| "id": "exportAssignment1" |}/*exportAssignment1_exportKeyword*/[|export|] = [|{| "isWriteAccess": false, "isDefinition": false, "contextRangeId": "exportAssignment1"|}x|];|]
// @filename: /main3.ts
[|{| "id": "varDecl3" |}const [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeId": "varDecl3" |}y|] = {};|]
[|{| "id": "exportAssignment2" |}/*exportAssignment2_exportKeyword*/[|export|] [|default|] [|{| "isWriteAccess": false, "isDefinition": false, "contextRangeId": "exportAssignment2"|}y|];|]
// @filename: /a.ts
export const a = 1;
// @filename: /b.ts
[|{| "id": "classDecl1" |}export default class [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeId": "classDecl1" |}B|] {}|]
// @filename: /c.ts
export const c = 1;
// @filename: /d.ts
[|{| "id": "classDecl2" |}export class [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeId": "classDecl2" |}D|] {}|]
// @filename: /e.ts
export const e1 = 1;
export const e2 = 2;
// @filename: /f.ts
export const f = 1;
// @filename: /g.ts
export const g = 1;
// @filename: /h.ts
export const h = 1;
// @filename: /i.ts
[|{| "id": "classDecl3" |}export class [|{| "isWriteAccess": true, "isDefinition": true, "contextRangeId": "classDecl3" |}I|] {}|]
// @filename: /j.ts
export const j1 = 1;
export const j2 = 2;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(
        t,
        &[
            "importEqualsDecl1_importKeyword".to_string(),
            "importEqualsDecl1_requireKeyword".to_string(),
            "importEqualsDecl2_importKeyword".to_string(),
            "importDecl1_importKeyword".to_string(),
            "importDecl1_typeKeyword".to_string(),
            "importDecl1_fromKeyword".to_string(),
            "importDecl2_importKeyword".to_string(),
            "importDecl2_typeKeyword".to_string(),
            "importDecl2_asKeyword".to_string(),
            "importDecl2_fromKeyword".to_string(),
            "importDecl3_importKeyword".to_string(),
            "importDecl3_typeKeyword".to_string(),
            "importDecl3_fromKeyword".to_string(),
            "importDecl4_importKeyword".to_string(),
            "importDecl4_typeKeyword".to_string(),
            "importDecl4_fromKeyword".to_string(),
            "importDecl4_asKeyword".to_string(),
            "importDecl5_importKeyword".to_string(),
            "exportDecl1_exportKeyword".to_string(),
            "exportDecl1_typeKeyword".to_string(),
            "exportDecl1_fromKeyword".to_string(),
            "exportDecl2_exportKeyword".to_string(),
            "exportDecl2_typeKeyword".to_string(),
            "exportDecl2_asKeyword".to_string(),
            "exportDecl2_fromKeyword".to_string(),
            "exportDecl3_exportKeyword".to_string(),
            "exportDecl3_typeKeyword".to_string(),
            "exportDecl3_fromKeyword".to_string(),
            "exportDecl4_exportKeyword".to_string(),
            "exportDecl4_typeKeyword".to_string(),
            "exportDecl4_fromKeyword".to_string(),
            "exportDecl4_asKeyword".to_string(),
            "exportDecl5_exportKeyword".to_string(),
            "exportDecl5_typeKeyword".to_string(),
            "exportDecl6_exportKeyword".to_string(),
            "exportDecl6_typeKeyword".to_string(),
            "exportDecl6_asKeyword".to_string(),
            "exportAssignment1_exportKeyword".to_string(),
            "exportAssignment2_exportKeyword".to_string(),
        ],
    );
    done();
}
