#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_organize_imports21() {
    let mut t = TestingT;
    run_test_organize_imports21(&mut t);
}

fn run_test_organize_imports21(t: &mut TestingT) {
    if should_skip_if_failing("TestOrganizeImports21") {
        return;
    }
    let content = r"// @filename: /a.ts
export interface LocationDefinitions {}
export interface PersonDefinitions {}
// @filename: /b.ts
export {
    /** @deprecated Use LocationDefinitions instead */
    LocationDefinitions as AddressDefinitions,
    LocationDefinitions,
    /** @deprecated Use PersonDefinitions instead */
    PersonDefinitions as NameDefinitions,
    PersonDefinitions,
} from './a';";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.go_to_file(t, "/b.ts");
    f.verify_organize_imports(
        t,
        r"export {
    /** @deprecated Use LocationDefinitions instead */
    LocationDefinitions as AddressDefinitions,
    LocationDefinitions,
    /** @deprecated Use PersonDefinitions instead */
    PersonDefinitions as NameDefinitions,
    PersonDefinitions
} from './a';
",
        "source.organizeImports",
        None,
    );
    done();
}
