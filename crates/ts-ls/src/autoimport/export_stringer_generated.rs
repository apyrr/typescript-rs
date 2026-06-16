use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum ExportSyntax {
    None = 0,
    Modifier = 1,
    Named = 2,
    DefaultModifier = 3,
    DefaultDeclaration = 4,
    Equals = 5,
    UMD = 6,
    Star = 7,
    CommonJSModuleExports = 8,
    CommonJSExportsProperty = 9,
}

impl ExportSyntax {
    pub fn as_str(self) -> &'static str {
        match self {
            ExportSyntax::None => "ExportSyntaxNone",
            ExportSyntax::Modifier => "ExportSyntaxModifier",
            ExportSyntax::Named => "ExportSyntaxNamed",
            ExportSyntax::DefaultModifier => "ExportSyntaxDefaultModifier",
            ExportSyntax::DefaultDeclaration => "ExportSyntaxDefaultDeclaration",
            ExportSyntax::Equals => "ExportSyntaxEquals",
            ExportSyntax::UMD => "ExportSyntaxUMD",
            ExportSyntax::Star => "ExportSyntaxStar",
            ExportSyntax::CommonJSModuleExports => "ExportSyntaxCommonJSModuleExports",
            ExportSyntax::CommonJSExportsProperty => "ExportSyntaxCommonJSExportsProperty",
        }
    }
}

impl Default for ExportSyntax {
    fn default() -> Self {
        ExportSyntax::None
    }
}

impl fmt::Display for ExportSyntax {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// PORT STATUS
//   source:     internal/ls/autoimport/export_stringer_generated.go (33 lines)
//   confidence: medium
//   todos:      merge ExportSyntax enum with full autoimport/export.go port
//   notes:      generated String behavior ported for Rust enum variants; not compile-validated
