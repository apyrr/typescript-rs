#![allow(non_snake_case)]
#![allow(unused_imports)]

use crate::generated_prelude::*;
use ts_core as core;
use ts_ls as lsutil;
use ts_lsproto as lsproto;
use ts_modulespecifiers as modulespecifiers;

#[test]
pub fn test_get_outlining_spans_depth_chained_calls() {
    let mut t = TestingT;
    run_test_get_outlining_spans_depth_chained_calls(&mut t);
}

fn run_test_get_outlining_spans_depth_chained_calls(t: &mut TestingT) {
    skip_if_failing(t);
    let content = r#"declare var router: any;
router
    .get[|("/", async(ctx) =>[|{
        ctx.body = "base";
    }|])|]
    .post[|("/a", async(ctx) =>[|{
        //a
    }|])|]
    .get[|("/", async(ctx) =>[|{
        ctx.body = "base";
    }|])|]
    .post[|("/a", async(ctx) =>[|{
        //a
    }|])|]
    .get[|("/", async(ctx) =>[|{
        ctx.body = "base";
    }|])|]
    .post[|("/a", async(ctx) =>[|{
        //a
    }|])|]
    .get[|("/", async(ctx) =>[|{
        ctx.body = "base";
    }|])|]
    .post[|("/a", async(ctx) =>[|{
        //a
    }|])|]
    .get[|("/", async(ctx) =>[|{
        ctx.body = "base";
    }|])|]
    .post[|("/a", async(ctx) =>[|{
        //a
    }|])|]
    .get[|("/", async(ctx) =>[|{
        ctx.body = "base";
    }|])|]
    .post[|("/a", async(ctx) =>[|{
        //a
    }|])|]
    .get[|("/", async(ctx) =>[|{
        ctx.body = "base";
    }|])|]
    .post[|("/a", async(ctx) =>[|{
        //a
    }|])|]
    .get[|("/", async(ctx) =>[|{
        ctx.body = "base";
    }|])|]
    .post[|("/a", async(ctx) =>[|{
        //a
    }|])|]
    .get[|("/", async(ctx) =>[|{
        ctx.body = "base";
    }|])|]
    .post[|("/a", async(ctx) =>[|{
        //a
    }|])|]
    .get[|("/", async(ctx) =>[|{
        ctx.body = "base";
    }|])|]
    .post[|("/a", async(ctx) =>[|{
        //a
    }|])|]
    .get[|("/", async(ctx) =>[|{
        ctx.body = "base";
    }|])|]
    .post[|("/a", async(ctx) =>[|{
        //a
    }|])|]
    .get[|("/", async(ctx) =>[|{
        ctx.body = "base";
    }|])|]
    .post[|("/a", async(ctx) =>[|{
        //a
    }|])|]
    .get[|("/", async(ctx) =>[|{
        ctx.body = "base";
    }|])|]
    .post[|("/a", async(ctx) =>[|{
        //a
    }|])|]
    .get[|("/", async(ctx) =>[|{
        ctx.body = "base";
    }|])|]
    .post[|("/a", async(ctx) =>[|{
        //a
    }|])|]
    .get[|("/", async(ctx) =>[|{
        ctx.body = "base";
    }|])|]
    .post[|("/a", async(ctx) =>[|{
        //a
    }|])|]
    .get[|("/", async(ctx) =>[|{
        ctx.body = "base";
    }|])|]
    .post[|("/a", async(ctx) =>[|{
        //a
    }|])|]
    .get[|("/", async(ctx) =>[|{
        ctx.body = "base";
    }|])|]
    .post[|("/a", async(ctx) =>[|{
        //a
    }|])|]
    .get[|("/", async(ctx) =>[|{
        ctx.body = "base";
    }|])|]
    .post[|("/a", async(ctx) =>[|{
        //a
    }|])|]"#;
    let (mut f, done) = new_fourslash(t, None /*capabilities*/, content.to_string());
    f.verify_outlining_spans_from_ranges(t);
    done();
}
