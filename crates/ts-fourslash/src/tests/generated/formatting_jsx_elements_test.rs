#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_jsx_elements() {
    let mut t = TestingT;
    run_test_formatting_jsx_elements(&mut t);
}

fn run_test_formatting_jsx_elements(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingJsxElements") {
        return;
    }
    let content = r#"//@Filename: file.tsx
function foo0() {
    return (
        <div className="commentBox" >
Hello, World!/*autoformat*/
/*indent*/
        </div>
    )
}

function foo1() {
    return (
        <div className="commentBox" data-id="test">
Hello, World!/*autoformat1*/
/*indent1*/
        </div>
    )
}

function foo2() {
    return (
        <div data-name="commentBox"
class1= {/*1*/
}>/*2*/
Hello, World!/*autoformat2*/
/*indent2*/
        </div>
    )
}
function foo3() {
    return (
        <jsx-element className="commentBox"
            class2= {/*3*/
            }>/*4*/
            Hello, World!/*autoformat3*/
        /*indent3*/
        </jsx-element>
    )
}
function foo4() {
    return (
        <jsx-element className="commentBox"
            class3= {/*5*/
            }/>/*6*/
    )
}

const bar = (
    <>
    /*fragmentChildIndent*/<p>text</p>
    </>
);

const bar2 = <>
    <p>text</p>
    /*fragmentClosingTagIndent*/</>;

(function () {
    return <div
className=""/*attrAutoformat*/
/*attrIndent*/
id={
"abc" + "cde"/*expressionAutoformat*/
/*expressionIndent*/
}
        >/*danglingBracketAutoformat*/
        </div>/*closingTagAutoformat*/
})

let h5 = <h5>
<span>/*childJsxElementAutoformat*/
/*childJsxElementIndent*/
<span></span>/*grandchildJsxElementAutoformat*/
</span>/*containedClosingTagAutoformat*/
</h5>;

<div>,{integer}</div>;/*commaInJsxElement*/
<div>,   {integer}</div>;/*commaInJsxElement2*/
<>,{integer}</>;/*commaInJsxFragment*/
<>,   {integer}</>;/*commaInJsxFragment2*/
<span>)</span>;/*closingParenInJsxElement*/
<span>)   </span>;/*closingParenInJsxElement2*/
<>)</>;/*closingParenInJsxFragment*/
<>)   </>;/*closingParenInJsxFragment2*/
<Router        routes      =        { 3 }   /      >;/*jsxExpressionSpaces*/
<Router routes={                (3)    } />;/*jsxExpressionSpaces2*/
<Router routes={() => {}}/*jsxExpressionSpaces3*/
/>;/*jsxDanglingSelfClosingToken*/"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "autoformat");
    f.verify_current_line_content(t, "            Hello, World!");
    f.go_to_marker(t, "indent");
    f.verify_indentation(t, 12);
    f.go_to_marker(t, "autoformat1");
    f.verify_current_line_content(t, "            Hello, World!");
    f.go_to_marker(t, "indent1");
    f.verify_indentation(t, 12);
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "            class1={");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "            }>");
    f.go_to_marker(t, "autoformat2");
    f.verify_current_line_content(t, "            Hello, World!");
    f.go_to_marker(t, "indent2");
    f.verify_indentation(t, 12);
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "            class2={");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "            }>");
    f.go_to_marker(t, "autoformat3");
    f.verify_current_line_content(t, "            Hello, World!");
    f.go_to_marker(t, "indent3");
    f.verify_indentation(t, 12);
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "            class3={");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "            } />");
    f.go_to_marker(t, "fragmentChildIndent");
    f.verify_current_line_content(t, "        <p>text</p>");
    f.go_to_marker(t, "fragmentClosingTagIndent");
    f.verify_current_line_content(t, "</>;");
    f.go_to_marker(t, "attrAutoformat");
    f.verify_current_line_content(t, "        className=\"\"");
    f.go_to_marker(t, "attrIndent");
    f.verify_indentation(t, 8);
    f.go_to_marker(t, "expressionAutoformat");
    f.verify_current_line_content(t, "            \"abc\" + \"cde\"");
    f.go_to_marker(t, "expressionIndent");
    f.verify_indentation(t, 12);
    f.go_to_marker(t, "danglingBracketAutoformat");
    f.verify_current_line_content(t, "    >");
    f.go_to_marker(t, "closingTagAutoformat");
    f.verify_current_line_content(t, "    </div>");
    f.go_to_marker(t, "childJsxElementAutoformat");
    f.verify_current_line_content(t, "    <span>");
    f.go_to_marker(t, "childJsxElementIndent");
    f.verify_indentation(t, 8);
    f.go_to_marker(t, "grandchildJsxElementAutoformat");
    f.verify_current_line_content(t, "        <span></span>");
    f.go_to_marker(t, "containedClosingTagAutoformat");
    f.verify_current_line_content(t, "    </span>");
    f.go_to_marker(t, "commaInJsxElement");
    f.verify_current_line_content(t, "<div>,{integer}</div>;");
    f.go_to_marker(t, "commaInJsxElement2");
    f.verify_current_line_content(t, "<div>,   {integer}</div>;");
    f.go_to_marker(t, "commaInJsxFragment");
    f.verify_current_line_content(t, "<>,{integer}</>;");
    f.go_to_marker(t, "commaInJsxFragment2");
    f.verify_current_line_content(t, "<>,   {integer}</>;");
    f.go_to_marker(t, "closingParenInJsxElement");
    f.verify_current_line_content(t, "<span>)</span>;");
    f.go_to_marker(t, "closingParenInJsxElement2");
    f.verify_current_line_content(t, "<span>)   </span>;");
    f.go_to_marker(t, "closingParenInJsxFragment");
    f.verify_current_line_content(t, "<>)</>;");
    f.go_to_marker(t, "closingParenInJsxFragment2");
    f.verify_current_line_content(t, "<>)   </>;");
    f.go_to_marker(t, "jsxExpressionSpaces");
    f.verify_current_line_content(t, "<Router routes={3} />;");
    f.go_to_marker(t, "jsxExpressionSpaces2");
    f.verify_current_line_content(t, "<Router routes={(3)} />;");
    f.go_to_marker(t, "jsxExpressionSpaces3");
    f.verify_current_line_content(t, "<Router routes={() => { }}");
    f.go_to_marker(t, "jsxDanglingSelfClosingToken");
    f.verify_current_line_content(t, "/>;");
    done();
}
