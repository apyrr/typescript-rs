#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_different_file_indirectly() {
    let mut t = TestingT;
    run_test_go_to_definition_different_file_indirectly(&mut t);
}

fn run_test_go_to_definition_different_file_indirectly(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionDifferentFileIndirectly") {
        return;
    }
    let content = r"// @Filename: Remote2.ts
var /*remoteVariableDefinition*/rem2Var;
function /*remoteFunctionDefinition*/rem2Fn() { }
class /*remoteClassDefinition*/rem2Cls { }
interface /*remoteInterfaceDefinition*/rem2Int{}
module /*remoteModuleDefinition*/rem2Mod { export var foo; }
// @Filename: Remote1.ts
var remVar;
function remFn() { }
class remCls { }
interface remInt{}
namespace remMod { export var foo; }
// @Filename: Definition.ts
/*remoteVariableReference*/rem2Var = 1;
/*remoteFunctionReference*/rem2Fn();
var rem2foo = new /*remoteClassReference*/rem2Cls();
class rem2fooCls implements /*remoteInterfaceReference*/rem2Int { }
var rem2fooVar = /*remoteModuleReference*/rem2Mod.foo;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "remoteVariableReference".to_string(),
            "remoteFunctionReference".to_string(),
            "remoteClassReference".to_string(),
            "remoteInterfaceReference".to_string(),
            "remoteModuleReference".to_string(),
        ],
    );
    done();
}
