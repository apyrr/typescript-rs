#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_different_file() {
    let mut t = TestingT;
    run_test_go_to_definition_different_file(&mut t);
}

fn run_test_go_to_definition_different_file(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"// @Filename: goToDefinitionDifferentFile_Definition.ts
var /*remoteVariableDefinition*/remoteVariable;
function /*remoteFunctionDefinition*/remoteFunction() { }
class /*remoteClassDefinition*/remoteClass { }
interface /*remoteInterfaceDefinition*/remoteInterface{ }
module /*remoteModuleDefinition*/remoteModule{ export var foo = 1;}
// @Filename: goToDefinitionDifferentFile_Consumption.ts
/*remoteVariableReference*/remoteVariable = 1;
/*remoteFunctionReference*/remoteFunction();
var foo = new /*remoteClassReference*/remoteClass();
class fooCls implements /*remoteInterfaceReference*/remoteInterface { }
var fooVar = /*remoteModuleReference*/remoteModule.foo;";
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
