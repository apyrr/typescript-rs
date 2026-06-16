use super::*;

#[test]
fn node_resolution_feature_constants_match_go_defaults() {
    assert_eq!(NODE_RESOLUTION_FEATURES_IMPORTS, 1 << 0);
    assert_eq!(NODE_RESOLUTION_FEATURES_SELF_NAME, 1 << 1);
    assert_eq!(NODE_RESOLUTION_FEATURES_EXPORTS, 1 << 2);
    assert_eq!(NODE_RESOLUTION_FEATURES_EXPORTS_PATTERN_TRAILERS, 1 << 3);
    assert_eq!(NODE_RESOLUTION_FEATURES_IMPORTS_PATTERN_ROOT, 1 << 4);
    assert_eq!(NODE_RESOLUTION_FEATURES_NONE, 0);
    assert_eq!(
        NODE_RESOLUTION_FEATURES_NODE16_DEFAULT,
        NODE_RESOLUTION_FEATURES_IMPORTS
            | NODE_RESOLUTION_FEATURES_SELF_NAME
            | NODE_RESOLUTION_FEATURES_EXPORTS
            | NODE_RESOLUTION_FEATURES_EXPORTS_PATTERN_TRAILERS
    );
    assert_eq!(
        NODE_RESOLUTION_FEATURES_NODE_NEXT_DEFAULT,
        NODE_RESOLUTION_FEATURES_ALL
    );
    assert_eq!(
        NODE_RESOLUTION_FEATURES_BUNDLER_DEFAULT,
        NODE_RESOLUTION_FEATURES_ALL
    );
}

#[test]
fn extensions_string_and_array_match_go_ordering() {
    let extensions =
        EXTENSIONS_TYPESCRIPT | EXTENSIONS_JAVASCRIPT | EXTENSIONS_DECLARATION | EXTENSIONS_JSON;

    assert_eq!(
        extensions_to_string(extensions),
        "TypeScript, JavaScript, Declaration, JSON"
    );
    assert_eq!(
        extensions_array(extensions),
        vec![
            ".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs", ".d.ts", ".d.mts",
            ".d.cts", ".json"
        ]
    );
}
