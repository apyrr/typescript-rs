//go:generate go tool golang.org/x/tools/cmd/stringer -type=ScriptKind -output=scriptkind_stringer_generated.go
//go:generate npx dprint fmt scriptkind_stringer_generated.go
// PORT NOTE: Rust stringer equivalent lives in scriptkind_stringer_generated.rs.

#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScriptKind(pub i32);

impl ScriptKind {
    #[allow(non_upper_case_globals)]
    pub const ScriptKindUnknown: ScriptKind = ScriptKind(0);
    #[allow(non_upper_case_globals)]
    pub const ScriptKindJS: ScriptKind = ScriptKind(1);
    #[allow(non_upper_case_globals)]
    pub const ScriptKindJSX: ScriptKind = ScriptKind(2);
    #[allow(non_upper_case_globals)]
    pub const ScriptKindTS: ScriptKind = ScriptKind(3);
    #[allow(non_upper_case_globals)]
    pub const ScriptKindTSX: ScriptKind = ScriptKind(4);
    #[allow(non_upper_case_globals)]
    pub const ScriptKindExternal: ScriptKind = ScriptKind(5);
    #[allow(non_upper_case_globals)]
    pub const ScriptKindJSON: ScriptKind = ScriptKind(6);
    /**
     * Used on extensions that doesn't define the ScriptKind but the content defines it.
     * Deferred extensions are going to be included in all project contexts.
     */
    #[allow(non_upper_case_globals)]
    pub const ScriptKindDeferred: ScriptKind = ScriptKind(7);

    #[allow(non_upper_case_globals)]
    pub const Unknown: ScriptKind = Self::ScriptKindUnknown;

    #[allow(non_upper_case_globals)]
    pub const JS: ScriptKind = Self::ScriptKindJS;

    #[allow(non_upper_case_globals)]
    pub const JSX: ScriptKind = Self::ScriptKindJSX;

    #[allow(non_upper_case_globals)]
    pub const TS: ScriptKind = Self::ScriptKindTS;

    #[allow(non_upper_case_globals)]
    pub const TSX: ScriptKind = Self::ScriptKindTSX;

    #[allow(non_upper_case_globals)]
    pub const External: ScriptKind = Self::ScriptKindExternal;

    #[allow(non_upper_case_globals)]
    pub const JSON: ScriptKind = Self::ScriptKindJSON;

    #[allow(non_upper_case_globals)]
    pub const Deferred: ScriptKind = Self::ScriptKindDeferred;

    #[allow(non_upper_case_globals)]
    pub const Js: ScriptKind = Self::JS;
    #[allow(non_upper_case_globals)]
    pub const Jsx: ScriptKind = Self::JSX;
    #[allow(non_upper_case_globals)]
    pub const Ts: ScriptKind = Self::TS;
    #[allow(non_upper_case_globals)]
    pub const Tsx: ScriptKind = Self::TSX;
    #[allow(non_upper_case_globals)]
    pub const Json: ScriptKind = Self::JSON;
}
