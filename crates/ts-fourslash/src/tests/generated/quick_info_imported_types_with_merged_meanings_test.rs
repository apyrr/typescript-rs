#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_quick_info_imported_types_with_merged_meanings() {
    let mut t = TestingT;
    run_test_quick_info_imported_types_with_merged_meanings(&mut t);
}

fn run_test_quick_info_imported_types_with_merged_meanings(t: &mut TestingT) {
    if should_skip_if_failing("TestQuickInfoImportedTypesWithMergedMeanings") {
        return;
    }
    let content = r"// @Filename: quickInfoImportedTypesWithMergedMeanings.ts
export namespace Original { }
export type Original<T> = () => T;
/** some docs */
export function Original() { }
// @Filename: transient.ts
export { Original/*1*/ } from './quickInfoImportedTypesWithMergedMeanings';
// @Filename: importer.ts
import { Original as /*2*/Alias } from './quickInfoImportedTypesWithMergedMeanings';
Alias/*3*/;
let x: Alias/*4*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_quick_info_at(t, "1", "(alias) function Original(): void\n(alias) type Original<T> = () => T\n(alias) namespace Original\nexport Original", "some docs");
    f.verify_quick_info_at(t, "2", "(alias) function Alias(): void\n(alias) type Alias<T> = () => T\n(alias) namespace Alias\nimport Alias", "some docs");
    f.verify_quick_info_at(
        t,
        "3",
        "(alias) function Alias(): void\n(alias) namespace Alias\nimport Alias",
        "some docs",
    );
    f.verify_quick_info_at(
        t,
        "4",
        "(alias) type Alias<T> = () => T\n(alias) namespace Alias\nimport Alias",
        "some docs",
    );
    done();
}
