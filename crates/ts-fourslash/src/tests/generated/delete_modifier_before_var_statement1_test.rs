#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_delete_modifier_before_var_statement1() {
    let mut t = TestingT;
    run_test_delete_modifier_before_var_statement1(&mut t);
}

fn run_test_delete_modifier_before_var_statement1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"

/////////////////////////////
/// Windows Script Host APIS
/////////////////////////////

declare var ActiveXObject: { new (s: string): any; };

interface ITextWriter {
    WriteLine(s): void;
}

declare var WScript: {
    Echo(s): void;
    StdErr: ITextWriter;
    Arguments: { length: number; Item(): string; };
    ScriptFullName: string;
    Quit(): number;
}
";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file_number(t, 0);
    f.go_to_position(t, 0);
    f.delete_at_caret(t, 100);
    f.go_to_position(t, 198);
    f.delete_at_caret(t, 16);
    f.go_to_position(t, 198);
    f.insert(t, "Item(): string; ");
    done();
}
