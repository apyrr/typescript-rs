#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_formatting_on_type_literal() {
    let mut t = TestingT;
    run_test_formatting_on_type_literal(&mut t);
}

fn run_test_formatting_on_type_literal(t: &mut TestingT) {
    if should_skip_if_failing("TestFormattingOnTypeLiteral") {
        return;
    }
    let content = r"function _uniteVertices<p extends string, a>(
    minority: Pinned<p, Vertex<a>>,
    majorityCounter: number,
    majority: Pinned<p, Vertex<a>>
): {
   /*start*/
        majorityCounter: number;
        vertecis: Pinned<p, {
            oldVertexId: VertexId;
            vertex: Vertex<a>;
        }>;
    /*end*/
    } {
}";
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.format_document(t, "");
    f.verify_current_file_content(
        t,
        r"function _uniteVertices<p extends string, a>(
    minority: Pinned<p, Vertex<a>>,
    majorityCounter: number,
    majority: Pinned<p, Vertex<a>>
): {

    majorityCounter: number;
    vertecis: Pinned<p, {
        oldVertexId: VertexId;
        vertex: Vertex<a>;
    }>;

} {
}",
    );
    f.go_to_marker(t, "start");
    f.verify_indentation(t, 4);
    f.go_to_marker(t, "end");
    f.verify_indentation(t, 4);
    done();
}
