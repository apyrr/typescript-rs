use ts_ast::Kind;

use crate::context::FormattingContext;

#[derive(Clone)]
pub struct RuleImpl {
    pub debug_name: String,
    pub context: Vec<ContextPredicate>,
    pub context_names: Vec<String>,
    pub action: RuleAction,
    pub flags: RuleFlags,
}

impl RuleImpl {
    pub fn action(&self) -> RuleAction {
        self.action
    }

    pub fn context(&self) -> &[ContextPredicate] {
        &self.context
    }

    pub fn context_names(&self) -> &[String] {
        &self.context_names
    }

    pub fn flags(&self) -> RuleFlags {
        self.flags
    }
}

impl std::fmt::Display for RuleImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.debug_name)
    }
}

#[derive(Clone)]
pub struct TokenRange {
    pub tokens: Vec<Kind>,
    pub is_specific: bool,
}

pub struct RuleSpec {
    pub left_token_range: TokenRange,
    pub right_token_range: TokenRange,
    pub rule: RuleImpl,
}

/**
 * A rule takes a two tokens (left/right) and a particular context
 * for which you're meant to look at them. You then declare what should the
 * whitespace annotation be between these tokens via the action param.
 *
 * @param debugName Name to print
 * @param left The left side of the comparison
 * @param right The right side of the comparison
 * @param context A set of filters to narrow down the space in which this formatter rule applies
 * @param action a declaration of the expected whitespace
 * @param flags whether the rule deletes a line or not, defaults to no-op
 */
pub fn rule(
    debug_name: String,
    left: TokenRangeInput,
    right: TokenRangeInput,
    context: Vec<ContextPredicate>,
    action: RuleAction,
    flags: Option<RuleFlags>,
) -> RuleSpec {
    let flag = flags.unwrap_or(RuleFlags::NONE);
    let left_range = to_token_range(left);
    let right_range = to_token_range(right);
    let rule = RuleImpl {
        debug_name,
        context,
        context_names: Vec::new(),
        action,
        flags: flag,
    };
    RuleSpec {
        left_token_range: left_range,
        right_token_range: right_range,
        rule,
    }
}

pub enum TokenRangeInput {
    Kind(Kind),
    Kinds(Vec<Kind>),
    TokenRange(TokenRange),
}

fn to_token_range(e: TokenRangeInput) -> TokenRange {
    match e {
        TokenRangeInput::Kind(t) => TokenRange {
            is_specific: true,
            tokens: vec![t],
        },
        TokenRangeInput::Kinds(t) => TokenRange {
            is_specific: true,
            tokens: t,
        },
        TokenRangeInput::TokenRange(t) => t,
    }
}

pub type ContextPredicate = fn(ctx: &mut FormattingContext) -> bool;

pub fn any_context() -> Vec<ContextPredicate> {
    Vec::new()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleAction(pub i32);

impl RuleAction {
    pub const NONE: RuleAction = RuleAction(0);
    pub const STOP_PROCESSING_SPACE_ACTIONS: RuleAction = RuleAction(1 << 0);
    pub const STOP_PROCESSING_TOKEN_ACTIONS: RuleAction = RuleAction(1 << 1);
    pub const INSERT_SPACE: RuleAction = RuleAction(1 << 2);
    pub const INSERT_NEW_LINE: RuleAction = RuleAction(1 << 3);
    pub const DELETE_SPACE: RuleAction = RuleAction(1 << 4);
    pub const DELETE_TOKEN: RuleAction = RuleAction(1 << 5);
    pub const INSERT_TRAILING_SEMICOLON: RuleAction = RuleAction(1 << 6);

    pub const STOP_ACTION: RuleAction = RuleAction(
        RuleAction::STOP_PROCESSING_SPACE_ACTIONS.0 | RuleAction::STOP_PROCESSING_TOKEN_ACTIONS.0,
    );
    pub const MODIFY_SPACE_ACTION: RuleAction = RuleAction(
        RuleAction::INSERT_SPACE.0 | RuleAction::INSERT_NEW_LINE.0 | RuleAction::DELETE_SPACE.0,
    );
    pub const MODIFY_TOKEN_ACTION: RuleAction =
        RuleAction(RuleAction::DELETE_TOKEN.0 | RuleAction::INSERT_TRAILING_SEMICOLON.0);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum RuleFlags {
    None = 0,
    CanDeleteNewLines = 1,
}

impl RuleFlags {
    pub const NONE: RuleFlags = RuleFlags::None;
    pub const CAN_DELETE_NEW_LINES: RuleFlags = RuleFlags::CanDeleteNewLines;
}
