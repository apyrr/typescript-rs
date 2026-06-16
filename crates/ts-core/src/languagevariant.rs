//go:generate go tool golang.org/x/tools/cmd/stringer -type=LanguageVariant -output=languagevariant_stringer_generated.go
//go:generate npx dprint fmt languagevariant_stringer_generated.go
// PORT NOTE: Rust stringer equivalent lives in languagevariant_stringer_generated.rs.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct LanguageVariant(pub i32);

impl LanguageVariant {
    #[allow(non_upper_case_globals)]
    pub const Standard: LanguageVariant = LanguageVariant(0);
    #[allow(non_upper_case_globals)]
    pub const JSX: LanguageVariant = LanguageVariant(1);
}
