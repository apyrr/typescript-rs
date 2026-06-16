#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_find_all_refs_nonexistent_property_no_crash1() {
    let mut t = TestingT;
    run_test_find_all_refs_nonexistent_property_no_crash1(&mut t);
}

fn run_test_find_all_refs_nonexistent_property_no_crash1(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"// @strict: true
// @allowJs: true
// @checkJs: true
// @filename: ./src/parser-input.js
export default () => {
  let input;

  const parserInput = {};

  parserInput.currentChar = () => input.charAt(parserInput.i);

  parserInput.end = () => {
    const isFinished = parserInput.i >= input.length;

    return {
      isFinished,
      furthest: parserInput.i,
    };
  };

  return parserInput;
};
// @filename: ./src/parser.js
import getParserInput from "./parser-input";

const Parser = function Parser(context, imports, fileInfo, currentIndex) {
  currentIndex = currentIndex || 0;
  let parsers;
  const parserInput = getParserInput();

  return {
    parserInput,
    parsers: (parsers = {
      variable: function () {
        let name;

        if (parserInput.currentChar() === "/*1*/@") {
          return name[1];
        }
      },
    }),
  };
};

export default Parser;"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_baseline_find_all_references(t, &["1".to_string()]);
    done();
}
