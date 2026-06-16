pub type EmitFlags = u32;

pub const EF_SINGLE_LINE: EmitFlags = 1 << 0; // The contents of this node should be emitted on a single line.
pub const EF_MULTI_LINE: EmitFlags = 1 << 1; // The contents of this node should be emitted on multiple lines.
pub const EF_NO_LEADING_SOURCE_MAP: EmitFlags = 1 << 2; // Do not emit a leading source map location for this node.
pub const EF_NO_TRAILING_SOURCE_MAP: EmitFlags = 1 << 3; // Do not emit a trailing source map location for this node.
pub const EF_NO_NESTED_SOURCE_MAPS: EmitFlags = 1 << 4; // Do not emit source map locations for children of this node.
pub const EF_NO_TOKEN_LEADING_SOURCE_MAPS: EmitFlags = 1 << 5; // Do not emit leading source map location for token nodes.
pub const EF_NO_TOKEN_TRAILING_SOURCE_MAPS: EmitFlags = 1 << 6; // Do not emit trailing source map location for token nodes.
pub const EF_NO_LEADING_COMMENTS: EmitFlags = 1 << 7; // Do not emit leading comments for this node.
pub const EF_NO_TRAILING_COMMENTS: EmitFlags = 1 << 8; // Do not emit trailing comments for this node.
pub const EF_NO_NESTED_COMMENTS: EmitFlags = 1 << 9; // Do not emit nested comments for children of this node.
pub const EF_HELPER_NAME: EmitFlags = 1 << 10; // The Identifier refers to an *unscoped* emit helper (one that is emitted at the top of the file)
pub const EF_EXPORT_NAME: EmitFlags = 1 << 11; // Ensure an export prefix is added for an identifier that points to an exported declaration with a local name (see SymbolFlags.ExportHasLocal).
pub const EF_LOCAL_NAME: EmitFlags = 1 << 12; // Ensure an export prefix is not added for an identifier that points to an exported declaration.
pub const EF_INDENTED: EmitFlags = 1 << 13; // Adds an explicit extra indentation level for class and function bodies when printing (used to match old emitter).
pub const EF_NO_INDENTATION: EmitFlags = 1 << 14; // Do not indent the node.
pub const EF_REUSE_TEMP_VARIABLE_SCOPE: EmitFlags = 1 << 15; // Reuse the existing temp variable scope during emit.
pub const EF_CUSTOM_PROLOGUE: EmitFlags = 1 << 16; // Treat the statement as if it were a prologue directive (NOTE: Prologue directives are *not* transformed).
pub const EF_NO_ASCII_ESCAPING: EmitFlags = 1 << 17; // When synthesizing nodes that lack an original node or textSourceNode, we want to write the text on the node with ASCII escaping substitutions.
pub const EF_EXTERNAL_HELPERS: EmitFlags = 1 << 18; // This source file has external helpers
pub const EF_START_ON_NEW_LINE: EmitFlags = 1 << 19; // Start this node on a new line
pub const EF_INDIRECT_CALL: EmitFlags = 1 << 20; // Emit CallExpression as an indirect call: `(0, f)()`
pub const EF_ASYNC_FUNCTION_BODY: EmitFlags = 1 << 21; // The node was originally an async function body.
pub const EF_NO_LEXICAL_ARGUMENTS: EmitFlags = 1 << 22; // Do not capture `arguments` for this arrow function. Set on arrows lowered from class static blocks, where `arguments` is an error; preserves Strada's emit behavior.
pub const EF_TRANSFORM_PRIVATE_STATIC_ELEMENTS: EmitFlags = 1 << 23; // Indicates static private elements in a file or class should be transformed regardless of --target (used by esDecorators transform).

pub const EF_NONE: EmitFlags = 0;
pub const EF_NO_SOURCE_MAP: EmitFlags = EF_NO_LEADING_SOURCE_MAP | EF_NO_TRAILING_SOURCE_MAP; // Do not emit a source map location for this node.
pub const EF_NO_TOKEN_SOURCE_MAPS: EmitFlags =
    EF_NO_TOKEN_LEADING_SOURCE_MAPS | EF_NO_TOKEN_TRAILING_SOURCE_MAPS; // Do not emit source map locations for tokens of this node.
pub const EF_NO_COMMENTS: EmitFlags = EF_NO_LEADING_COMMENTS | EF_NO_TRAILING_COMMENTS; // Do not emit comments for this node.
