use crate::{get_default_capabilities, new_fourslash, FoldingRangeLineExpected, TestingT};

pub fn test_folding_range_line_folding_only(t: &mut TestingT) {
    let content = r#"if (EMPTY_TAGs.has(tag)) {
  output += "/>";
} else {
  output += ">";

  if (!html && kidcount > 0) {
    //
  }
}

export function use<T>(ctx: any): T | undefined {
  //
}"#;
    let mut capabilities = get_default_capabilities();
    capabilities.text_document.folding_range.line_folding_only = true;
    capabilities.text_document.folding_range.folding_range.collapsed_text = true;
    let (mut f, done) = new_fourslash(t, Some(capabilities), content.to_string());

    // With lineFoldingOnly, end lines should be adjusted so closing brackets stay visible.
    // Line 0: if (EMPTY_TAGs.has(tag)) {
    // Line 1:   output += "/>";
    // Line 2: } else {
    // Line 3:   output += ">";
    // Line 4:
    // Line 5:   if (!html && kidcount > 0) {
    // Line 6:     //
    // Line 7:   }
    // Line 8: }
    // Line 9:
    // Line 10: export function use<T>(ctx: any): T | undefined {
    // Line 11:   //
    // Line 12: }
    f.verify_folding_range_lines(
        t,
        &[
            FoldingRangeLineExpected { start_line: 0, end_line: 1 },   // if block: end adjusted from line 2 to 1
            FoldingRangeLineExpected { start_line: 2, end_line: 7 },   // else block: end adjusted from line 8 to 7
            FoldingRangeLineExpected { start_line: 5, end_line: 6 },   // inner if block: end adjusted from line 7 to 6
            FoldingRangeLineExpected { start_line: 10, end_line: 11 }, // function: end adjusted from line 12 to 11
        ],
    );
    done();
}

pub fn test_folding_range_line_folding_only_with_regions(t: &mut TestingT) {
    let content = r#"// #region MyRegion
const x = 1;
function foo() {
  return x;
}
// #endregion

// #region Outer
const y = 2;
// #region Inner
const z = 3;
// #endregion
// #endregion"#;
    let mut capabilities = get_default_capabilities();
    capabilities.text_document.folding_range.line_folding_only = true;
    capabilities.text_document.folding_range.folding_range.collapsed_text = true;
    let (mut f, done) = new_fourslash(t, Some(capabilities), content.to_string());

    // Line 0: // #region MyRegion
    // Line 1: const x = 1;
    // Line 2: function foo() {
    // Line 3:   return x;
    // Line 4: }
    // Line 5: // #endregion
    // Line 6:
    // Line 7: // #region Outer
    // Line 8: const y = 2;
    // Line 9: // #region Inner
    // Line 10: const z = 3;
    // Line 11: // #endregion
    // Line 12: // #endregion
    f.verify_folding_range_lines(
        t,
        &[
            FoldingRangeLineExpected { start_line: 0, end_line: 5 },  // #region MyRegion: NOT adjusted (ends with "n", not a closing pair)
            FoldingRangeLineExpected { start_line: 2, end_line: 3 },  // function foo() block: end adjusted from line 4 to 3
            FoldingRangeLineExpected { start_line: 7, end_line: 12 }, // #region Outer: NOT adjusted
            FoldingRangeLineExpected { start_line: 9, end_line: 11 }, // #region Inner: NOT adjusted
        ],
    );
    done();
}

