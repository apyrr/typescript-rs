#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_named_export_import() {
    let mut t = TestingT;
    run_test_format_named_export_import(&mut t);
}

fn run_test_format_named_export_import(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"/*selectionStart*/
export {   x, y    as     yy, z       }       from        "foo"/*export1*/
export{x, y as yy, z}from"bar"/*export2*/

export
/*exportOpenBrace*/{x,/*exportSpecifier1*/
y as  yy, z/*exportSpecifier2*/ }/*exportCloseBrace*/
  from/*fromKeywordAutoformat*/
/*fromKeywordIndent*/
"foo"/*exportDir*/

import {x, y as yy, z}from   "baz"/*import1*/

import/*importOpenBrace*/{x,/*importSpecifier1*/
y
as yy,/*importSpecifier2*/
z}/*importCloseBrace*/
from   "wow"/*importDir*/
/*selectionEnd*/

export/*formatOnEnter*/{/*formatOnEnterOpenBrace*/
/*differentLineIndent*/x/*differentLineAutoformat*/
} from "abc"

export {
/*incompleteExportDeclIndent*/
/*incompleteExportDeclIndent2*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_selection(t, "selectionStart", "selectionEnd");
    f.go_to_marker(t, "export1");
    f.verify_current_line_content(t, "export { x, y as yy, z } from \"foo\"");
    f.go_to_marker(t, "export2");
    f.verify_current_line_content(t, "export { x, y as yy, z } from \"bar\"");
    f.go_to_marker(t, "exportOpenBrace");
    f.verify_current_line_content(t, "export {");
    f.go_to_marker(t, "exportSpecifier1");
    f.verify_current_line_content(t, "    x,");
    f.go_to_marker(t, "exportSpecifier2");
    f.verify_current_line_content(t, "    y as yy, z");
    f.go_to_marker(t, "exportCloseBrace");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "fromKeywordAutoformat");
    f.verify_current_line_content(t, "    from");
    f.go_to_marker(t, "fromKeywordIndent");
    f.verify_indentation(t, 4);
    f.go_to_marker(t, "exportDir");
    f.verify_current_line_content(t, "    \"foo\"");
    f.go_to_marker(t, "import1");
    f.verify_current_line_content(t, "import { x, y as yy, z } from \"baz\"");
    f.go_to_marker(t, "importOpenBrace");
    f.verify_current_line_content(t, "import {");
    f.go_to_marker(t, "importSpecifier1");
    f.verify_current_line_content(t, "    x,");
    f.go_to_marker(t, "importSpecifier2");
    f.verify_current_line_content(t, "        as yy,");
    f.go_to_marker(t, "importCloseBrace");
    f.verify_current_line_content(t, "}");
    f.go_to_marker(t, "importDir");
    f.verify_current_line_content(t, "    from \"wow\"");
    f.go_to_marker(t, "formatOnEnter");
    f.insert_line(t, "");
    f.go_to_marker(t, "formatOnEnterOpenBrace");
    f.verify_current_line_content(t, "{");
    f.go_to_marker(t, "differentLineIndent");
    f.verify_indentation(t, 4);
    f.insert_line(t, "");
    f.go_to_marker(t, "differentLineAutoformat");
    f.verify_current_line_content(t, "    x");
    f.go_to_marker(t, "incompleteExportDeclIndent");
    f.verify_indentation(t, 4);
    f.insert(t, "} from");
    f.go_to_marker(t, "incompleteExportDeclIndent2");
    f.verify_indentation(t, 4);
    done();
}
