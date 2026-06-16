#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_declaration_maps_go_to_definition_same_name_different_directory() {
    let mut t = TestingT;
    run_test_declaration_maps_go_to_definition_same_name_different_directory(&mut t);
}

fn run_test_declaration_maps_go_to_definition_same_name_different_directory(t: &mut TestingT) {
    if should_skip_if_failing("TestDeclarationMapsGoToDefinitionSameNameDifferentDirectory") {
        return;
    }
    let content = r#"// @Filename: BaseClass/Source.d.ts
declare class Control {
    constructor();
    /** this is a super var */
    myVar: boolean | 'yeah';
}
//# sourceMappingURL=Source.d.ts.map
// @Filename: BaseClass/Source.d.ts.map
{"version":3,"file":"Source.d.ts","sourceRoot":"","sources":["Source.ts"],"names":[],"mappings":"AAAA,cAAM,OAAO;;IAIT,0BAA0B;IACnB,KAAK,EAAE,OAAO,GAAG,MAAM,CAAQ;CACzC"}
// @Filename: BaseClass/Source.ts
class /*2*/Control{
    constructor(){
        return;
    }
    /** this is a super var */
    public /*4*/myVar: boolean | 'yeah' = true;
}
// @Filename: tsbase.json
{
    "$schema": "http://json.schemastore.org/tsconfig",
    "compileOnSave": true,
    "compilerOptions": {
      "lib": ["es5"],
      "strict": false,
      "sourceMap": true,
      "declaration": true,
      "declarationMap": true
    }
  }
// @Filename: buttonClass/tsconfig.json
{
    "extends": "../tsbase.json",
    "compilerOptions": {
      "outFile": "Source.js"
    },
    "files": [
      "Source.ts"
    ],
    "include": [
      "../BaseClass/Source.d.ts"
    ]
  }
// @Filename: buttonClass/Source.ts
// I cannot F12 navigate to Control
//                   vvvvvvv
class Button extends [|/*1*/Control|] {
    public myFunction() {
        // I cannot F12 navigate to myVar
        //              vvvvv
        if (typeof this.[|/*3*/myVar|] === 'boolean') {
            this.myVar;
        } else {
            this.myVar.toLocaleUpperCase();
        }
    }
}"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.mark_test_as_strada_server();
    f.verify_baseline_go_to_definition(t, &["1".to_string(), "3".to_string()]);
    done();
}
