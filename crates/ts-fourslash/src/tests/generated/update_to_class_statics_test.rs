#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_update_to_class_statics() {
    let mut t = TestingT;
    run_test_update_to_class_statics(&mut t);
}

fn run_test_update_to_class_statics(t: &mut TestingT) {
    if should_skip_if_failing("TestUpdateToClassStatics") {
        return;
    }
    let content = r"namespace TypeScript {
    export class PullSymbol {}
    export class Diagnostic {}
    export class SymbolAndDiagnostics<TSymbol extends PullSymbol> {
        constructor(public symbol: TSymbol,
            public diagnostics: Diagnostic) {
        }
        /**/
        public static create<TSymbol extends PullSymbol>(symbol: TSymbol, diagnostics: Diagnostic): SymbolAndDiagnostics<TSymbol> {
            return new SymbolAndDiagnostics<TSymbol>(symbol, diagnostics);
        }
    }
}
namespace TypeScript {
    var x : TypeScript.SymbolAndDiagnostics;
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_marker(t, "");
    f.insert(t, "someNewProperty = 0;");
    done();
}
