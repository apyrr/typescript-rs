#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_go_to_definition_same_file() {
    let mut t = TestingT;
    run_test_go_to_definition_same_file(&mut t);
}

fn run_test_go_to_definition_same_file(t: &mut TestingT) {
    if should_skip_if_failing("TestGoToDefinitionSameFile") {
        return;
    }
    let content = r"var /*localVariableDefinition*/localVariable;
function /*localFunctionDefinition*/localFunction() { }
class /*localClassDefinition*/localClass { }
interface /*localInterfaceDefinition*/localInterface{ }
module /*localModuleDefinition*/localModule{ export var foo = 1;}


/*localVariableReference*/localVariable = 1;
/*localFunctionReference*/localFunction();
var foo = new /*localClassReference*/localClass();
class fooCls implements /*localInterfaceReference*/localInterface { }
var fooVar = /*localModuleReference*/localModule.foo;";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_go_to_definition(
        t,
        &[
            "localVariableReference".to_string(),
            "localFunctionReference".to_string(),
            "localClassReference".to_string(),
            "localInterfaceReference".to_string(),
            "localModuleReference".to_string(),
        ],
    );
    done();
}
