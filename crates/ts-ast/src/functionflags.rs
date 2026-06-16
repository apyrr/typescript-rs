use crate::*;

pub type FunctionFlags = u32;

pub const FUNCTION_FLAGS_NORMAL: FunctionFlags = 0;
pub const FUNCTION_FLAGS_GENERATOR: FunctionFlags = 1 << 0;
pub const FUNCTION_FLAGS_ASYNC: FunctionFlags = 1 << 1;
pub const FUNCTION_FLAGS_INVALID: FunctionFlags = 1 << 2;
pub const FUNCTION_FLAGS_ASYNC_GENERATOR: FunctionFlags =
    FUNCTION_FLAGS_ASYNC | FUNCTION_FLAGS_GENERATOR;

pub fn get_function_flags(store: &AstStore, node: Option<Node>) -> FunctionFlags {
    let Some(node) = node else {
        return FUNCTION_FLAGS_INVALID;
    };
    let data = store.body_data(node);
    let Some(data) = data else {
        return FUNCTION_FLAGS_INVALID;
    };
    let mut flags = FUNCTION_FLAGS_NORMAL;
    match store.kind(node) {
        Kind::FunctionDeclaration | Kind::FunctionExpression | Kind::MethodDeclaration => {
            if data.asterisk_token.is_some() {
                flags |= FUNCTION_FLAGS_GENERATOR;
            }
            if has_syntactic_modifier(store, node, ModifierFlags::ASYNC) {
                flags |= FUNCTION_FLAGS_ASYNC;
            }
        }
        Kind::ArrowFunction if has_syntactic_modifier(store, node, ModifierFlags::ASYNC) => {
            flags |= FUNCTION_FLAGS_ASYNC;
        }
        _ => {}
    }
    if data.body.is_none() {
        flags |= FUNCTION_FLAGS_INVALID;
    }
    flags
}
