pub mod constenum;

use crate::{TransformOptions, Transformer};

pub fn new_const_enum_inlining_transformer(opts: &TransformOptions) -> Transformer {
    if opts.compiler_options.get_isolated_modules() {
        ts_debug::fail("const enums are not inlined under isolated modules");
    }

    let mut tx = Transformer::default();
    tx.new_source_file_transformer(
        crate::SourceFileTransformer::ConstEnumInlining {
            compiler_options: opts.compiler_options.clone(),
            facts: opts
                .const_enum_inlining_facts
                .clone()
                .expect("ConstEnumInliningTransformer requires resolver facts"),
        },
        Some(opts.context.fork()),
    );
    tx
}
