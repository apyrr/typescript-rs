#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_on_value_symbol_without_export_with_same_name_export_symbol() {
    let mut t = TestingT;
    run_test_quick_info_on_value_symbol_without_export_with_same_name_export_symbol(&mut t);
}

fn run_test_quick_info_on_value_symbol_without_export_with_same_name_export_symbol(
    t: &mut TestingT,
) {
    if should_skip_if_failing("TestQuickInfoOnValueSymbolWithoutExportWithSameNameExportSymbol") {
        return;
    }
    let content = r"// @strict: true

declare function num(): number
const /*1*/Unit = num()
export type Unit = number
const value = /*2*/Unit

function Fn() {}
export type Fn = () => void
/*3*/Fn()

// repro from #41897
const /*4*/X = 1;
export interface X {}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "const Unit: number", "");
    f.verify_quick_info_at(t, "2", "const Unit: number", "");
    f.verify_quick_info_at(t, "3", "function Fn(): void", "");
    f.verify_quick_info_at(t, "4", "const X: 1", "");
    done();
}
