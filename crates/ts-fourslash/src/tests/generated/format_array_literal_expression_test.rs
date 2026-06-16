#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_format_array_literal_expression() {
    let mut t = TestingT;
    run_test_format_array_literal_expression(&mut t);
}

fn run_test_format_array_literal_expression(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r"export let Things = [{
    Hat: 'hat', /*1*/
    Glove: 'glove',
    Umbrella: 'umbrella'
},{/*2*/
        Salad: 'salad', /*3*/
        Burrito: 'burrito',
        Pie: 'pie'
    }];/*4*/

export let Things2 = [
{
    Hat: 'hat', /*5*/
    Glove: 'glove',
    Umbrella: 'umbrella'
}/*6*/,
    {
        Salad: 'salad', /*7*/
        Burrito: ['burrito', 'carne asada', 'tinga de res', 'tinga de pollo'], /*8*/
        Pie: 'pie'
    }];/*9*/";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.go_to_marker(t, "1");
    f.verify_current_line_content(t, "    Hat: 'hat',");
    f.go_to_marker(t, "2");
    f.verify_current_line_content(t, "}, {");
    f.go_to_marker(t, "3");
    f.verify_current_line_content(t, "    Salad: 'salad',");
    f.go_to_marker(t, "4");
    f.verify_current_line_content(t, "}];");
    f.go_to_marker(t, "5");
    f.verify_current_line_content(t, "        Hat: 'hat',");
    f.go_to_marker(t, "6");
    f.verify_current_line_content(t, "    },");
    f.go_to_marker(t, "7");
    f.verify_current_line_content(t, "        Salad: 'salad',");
    f.go_to_marker(t, "8");
    f.verify_current_line_content(
        t,
        "        Burrito: ['burrito', 'carne asada', 'tinga de res', 'tinga de pollo'],",
    );
    f.go_to_marker(t, "9");
    f.verify_current_line_content(t, "    }];");
    done();
}
