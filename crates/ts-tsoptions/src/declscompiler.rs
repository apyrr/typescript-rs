use std::sync::LazyLock;

use crate::{CommandLineOption, CommandLineOptionKind, DefaultValueDescription, Tristate};

static OPTIONS_DECLARATIONS: LazyLock<Vec<CommandLineOption>> = LazyLock::new(|| {
    let mut common_options_with_build = crate::declsbuild::common_options_with_build();
    let common_names: Vec<_> = common_options_with_build
        .iter()
        .map(|option| option.name.clone())
        .collect();
    let names = [
        ("all", CommandLineOptionKind::Boolean),
        ("allowArbitraryExtensions", CommandLineOptionKind::Boolean),
        ("allowImportingTsExtensions", CommandLineOptionKind::Boolean),
        ("allowJs", CommandLineOptionKind::Boolean),
        (
            "allowSyntheticDefaultImports",
            CommandLineOptionKind::Boolean,
        ),
        ("allowUmdGlobalAccess", CommandLineOptionKind::Boolean),
        ("allowUnreachableCode", CommandLineOptionKind::Boolean),
        ("allowUnusedLabels", CommandLineOptionKind::Boolean),
        ("alwaysStrict", CommandLineOptionKind::Boolean),
        ("baseUrl", CommandLineOptionKind::String),
        ("charset", CommandLineOptionKind::String),
        ("checkJs", CommandLineOptionKind::Boolean),
        ("composite", CommandLineOptionKind::Boolean),
        ("customConditions", CommandLineOptionKind::List),
        ("declaration", CommandLineOptionKind::Boolean),
        ("declarationDir", CommandLineOptionKind::String),
        ("declarationMap", CommandLineOptionKind::Boolean),
        ("diagnostics", CommandLineOptionKind::Boolean),
        (
            "disableReferencedProjectLoad",
            CommandLineOptionKind::Boolean,
        ),
        ("disableSizeLimit", CommandLineOptionKind::Boolean),
        ("disableSolutionSearching", CommandLineOptionKind::Boolean),
        (
            "disableSourceOfProjectReferenceRedirect",
            CommandLineOptionKind::Boolean,
        ),
        ("downlevelIteration", CommandLineOptionKind::Boolean),
        ("emitBOM", CommandLineOptionKind::Boolean),
        ("emitDeclarationOnly", CommandLineOptionKind::Boolean),
        ("emitDecoratorMetadata", CommandLineOptionKind::Boolean),
        ("erasableSyntaxOnly", CommandLineOptionKind::Boolean),
        ("esModuleInterop", CommandLineOptionKind::Boolean),
        ("exactOptionalPropertyTypes", CommandLineOptionKind::Boolean),
        ("experimentalDecorators", CommandLineOptionKind::Boolean),
        ("explainFiles", CommandLineOptionKind::Boolean),
        ("extendedDiagnostics", CommandLineOptionKind::Boolean),
        (
            "forceConsistentCasingInFileNames",
            CommandLineOptionKind::Boolean,
        ),
        ("generateCpuProfile", CommandLineOptionKind::String),
        ("generateTrace", CommandLineOptionKind::String),
        ("help", CommandLineOptionKind::Boolean),
        ("importHelpers", CommandLineOptionKind::Boolean),
        ("importsNotUsedAsValues", CommandLineOptionKind::Enum),
        ("incremental", CommandLineOptionKind::Boolean),
        ("ignoreConfig", CommandLineOptionKind::Boolean),
        ("ignoreDeprecations", CommandLineOptionKind::String),
        ("init", CommandLineOptionKind::Boolean),
        ("inlineSourceMap", CommandLineOptionKind::Boolean),
        ("inlineSources", CommandLineOptionKind::Boolean),
        ("isolatedDeclarations", CommandLineOptionKind::Boolean),
        ("isolatedModules", CommandLineOptionKind::Boolean),
        ("jsx", CommandLineOptionKind::Enum),
        ("jsxFactory", CommandLineOptionKind::String),
        ("jsxFragmentFactory", CommandLineOptionKind::String),
        ("jsxImportSource", CommandLineOptionKind::String),
        ("keyofStringsOnly", CommandLineOptionKind::Boolean),
        ("lib", CommandLineOptionKind::List),
        ("libReplacement", CommandLineOptionKind::Boolean),
        ("listEmittedFiles", CommandLineOptionKind::Boolean),
        ("listFiles", CommandLineOptionKind::Boolean),
        ("listFilesOnly", CommandLineOptionKind::Boolean),
        ("locale", CommandLineOptionKind::String),
        ("mapRoot", CommandLineOptionKind::String),
        ("maxNodeModuleJsDepth", CommandLineOptionKind::Number),
        ("module", CommandLineOptionKind::Enum),
        ("moduleDetection", CommandLineOptionKind::Enum),
        ("moduleResolution", CommandLineOptionKind::Enum),
        ("moduleSuffixes", CommandLineOptionKind::List),
        ("newLine", CommandLineOptionKind::Enum),
        ("noCheck", CommandLineOptionKind::Boolean),
        ("noEmit", CommandLineOptionKind::Boolean),
        ("noEmitHelpers", CommandLineOptionKind::Boolean),
        ("noEmitOnError", CommandLineOptionKind::Boolean),
        ("noErrorTruncation", CommandLineOptionKind::Boolean),
        ("noFallthroughCasesInSwitch", CommandLineOptionKind::Boolean),
        ("noImplicitAny", CommandLineOptionKind::Boolean),
        ("noImplicitOverride", CommandLineOptionKind::Boolean),
        ("noImplicitReturns", CommandLineOptionKind::Boolean),
        ("noImplicitThis", CommandLineOptionKind::Boolean),
        ("noImplicitUseStrict", CommandLineOptionKind::Boolean),
        ("noLib", CommandLineOptionKind::Boolean),
        (
            "noPropertyAccessFromIndexSignature",
            CommandLineOptionKind::Boolean,
        ),
        ("noResolve", CommandLineOptionKind::Boolean),
        ("noStrictGenericChecks", CommandLineOptionKind::Boolean),
        ("noUncheckedIndexedAccess", CommandLineOptionKind::Boolean),
        (
            "noUncheckedSideEffectImports",
            CommandLineOptionKind::Boolean,
        ),
        ("noUnusedLocals", CommandLineOptionKind::Boolean),
        ("noUnusedParameters", CommandLineOptionKind::Boolean),
        ("out", CommandLineOptionKind::String),
        ("outDir", CommandLineOptionKind::String),
        ("outFile", CommandLineOptionKind::String),
        ("paths", CommandLineOptionKind::Object),
        ("plugins", CommandLineOptionKind::List),
        ("preserveConstEnums", CommandLineOptionKind::Boolean),
        ("preserveSymlinks", CommandLineOptionKind::Boolean),
        ("preserveValueImports", CommandLineOptionKind::Boolean),
        ("preserveWatchOutput", CommandLineOptionKind::Boolean),
        ("pretty", CommandLineOptionKind::Boolean),
        ("project", CommandLineOptionKind::String),
        ("reactNamespace", CommandLineOptionKind::String),
        ("removeComments", CommandLineOptionKind::Boolean),
        ("resolveJsonModule", CommandLineOptionKind::Boolean),
        ("resolvePackageJsonExports", CommandLineOptionKind::Boolean),
        ("resolvePackageJsonImports", CommandLineOptionKind::Boolean),
        (
            "rewriteRelativeImportExtensions",
            CommandLineOptionKind::Boolean,
        ),
        ("rootDir", CommandLineOptionKind::String),
        ("rootDirs", CommandLineOptionKind::List),
        ("showConfig", CommandLineOptionKind::Boolean),
        ("skipDefaultLibCheck", CommandLineOptionKind::Boolean),
        ("skipLibCheck", CommandLineOptionKind::Boolean),
        ("sourceMap", CommandLineOptionKind::Boolean),
        ("sourceRoot", CommandLineOptionKind::String),
        ("strict", CommandLineOptionKind::Boolean),
        ("strictBindCallApply", CommandLineOptionKind::Boolean),
        (
            "strictBuiltinIteratorReturn",
            CommandLineOptionKind::Boolean,
        ),
        ("strictFunctionTypes", CommandLineOptionKind::Boolean),
        ("strictNullChecks", CommandLineOptionKind::Boolean),
        (
            "strictPropertyInitialization",
            CommandLineOptionKind::Boolean,
        ),
        ("stableTypeOrdering", CommandLineOptionKind::Boolean),
        ("stripInternal", CommandLineOptionKind::Boolean),
        (
            "suppressExcessPropertyErrors",
            CommandLineOptionKind::Boolean,
        ),
        (
            "suppressImplicitAnyIndexErrors",
            CommandLineOptionKind::Boolean,
        ),
        ("target", CommandLineOptionKind::Enum),
        ("traceResolution", CommandLineOptionKind::Boolean),
        ("tsBuildInfoFile", CommandLineOptionKind::String),
        ("typeRoots", CommandLineOptionKind::List),
        ("types", CommandLineOptionKind::List),
        ("useDefineForClassFields", CommandLineOptionKind::Boolean),
        ("useUnknownInCatchVariables", CommandLineOptionKind::Boolean),
        ("verbatimModuleSyntax", CommandLineOptionKind::Boolean),
        ("version", CommandLineOptionKind::Boolean),
        ("watch", CommandLineOptionKind::Boolean),
    ];
    let mut compiler_options: Vec<_> = names
        .into_iter()
        .filter(|(name, _)| !common_names.iter().any(|common_name| common_name == name))
        .map(|(name, kind)| {
            let mut option = CommandLineOption::new(name, kind);
            match name {
                "target" => {
                    option.short_name = "t".to_owned();
                    option.affects_source_file = true;
                    option.affects_module_resolution = true;
                    option.affects_emit = true;
                    option.affects_build_info = true;
                }
                "module" => {
                    option.short_name = "m".to_owned();
                    option.affects_module_resolution = true;
                    option.affects_emit = true;
                    option.affects_build_info = true;
                }
                "lib" => {
                    option.affects_program_structure = true;
                    option.transpile_option_value = Tristate::Unknown;
                }
                "allowJs" => {
                    option.allow_js_flag = true;
                    option.affects_build_info = true;
                }
                "checkJs" => {
                    option.affects_module_resolution = true;
                    option.affects_semantic_diagnostics = true;
                    option.affects_build_info = true;
                }
                "jsx" => {
                    option.affects_source_file = true;
                    option.affects_emit = true;
                    option.affects_build_info = true;
                    option.affects_module_resolution = true;
                    // The checker emits an error when it sees JSX but this option is not set in compilerOptions.
                    // This is effectively a semantic error, so mark this option as affecting semantic diagnostics
                    // so we know to refresh errors when this option is changed.
                    option.affects_semantic_diagnostics = true;
                }
                "declarationDir" | "outFile" => {
                    option.affects_emit = true;
                    option.affects_build_info = true;
                    option.affects_declaration_path = true;
                    option.is_file_path = true;
                    option.transpile_option_value = Tristate::Unknown;
                }
                "outDir" | "rootDir" => {
                    option.affects_emit = true;
                    option.affects_build_info = true;
                    option.affects_declaration_path = true;
                    option.is_file_path = true;
                }
                "composite" => {
                    // Not setting affectsEmit because we calculate this flag might not affect full emit
                    option.affects_build_info = true;
                    option.transpile_option_value = Tristate::Unknown;
                }
                "tsBuildInfoFile" => {
                    option.affects_emit = true;
                    option.affects_build_info = true;
                    option.is_file_path = true;
                    option.transpile_option_value = Tristate::Unknown;
                }
                "removeComments" | "downlevelIteration" | "emitBOM" | "newLine"
                | "stripInternal" | "noEmitHelpers" | "preserveConstEnums" => {
                    option.affects_emit = true;
                    option.affects_build_info = true;
                }
                "importHelpers" => {
                    option.affects_emit = true;
                    option.affects_build_info = true;
                    option.affects_source_file = true;
                }
                "isolatedModules" => {
                    option.transpile_option_value = Tristate::True;
                }
                "moduleDetection" => {
                    option.affects_source_file = true;
                    option.affects_module_resolution = true;
                }
                "moduleResolution" => {
                    option.affects_module_resolution = true;
                }
                "baseUrl" => {
                    option.affects_module_resolution = true;
                    option.is_file_path = true;
                }
                "paths" => {
                    // this option can only be specified in tsconfig.json
                    // use type = object to copy the value as-is
                    option.affects_module_resolution = true;
                    option.allow_config_dir_template_substitution = true;
                    option.transpile_option_value = Tristate::Unknown;
                }
                "rootDirs" => {
                    // this option can only be specified in tsconfig.json
                    // use type = object to copy the value as-is
                    option.affects_module_resolution = true;
                    option.allow_config_dir_template_substitution = true;
                    option.transpile_option_value = Tristate::Unknown;
                }
                "typeRoots" => {
                    option.affects_module_resolution = true;
                    option.allow_config_dir_template_substitution = true;
                }
                "types" => {
                    option.affects_program_structure = true;
                    option.transpile_option_value = Tristate::Unknown;
                }
                "resolveJsonModule" => {
                    option.affects_module_resolution = true;
                }
                "resolvePackageJsonExports" | "resolvePackageJsonImports" => {
                    option.affects_module_resolution = true;
                }
                "customConditions" => {
                    option.affects_module_resolution = true;
                }
                "allowArbitraryExtensions" | "libReplacement" | "disableSizeLimit" => {
                    option.affects_program_structure = true;
                }
                "allowImportingTsExtensions" | "rewriteRelativeImportExtensions" => {
                    option.affects_semantic_diagnostics = true;
                    option.affects_build_info = true;
                }
                "allowSyntheticDefaultImports" => {
                    option.affects_semantic_diagnostics = true;
                    option.affects_build_info = true;
                }
                "esModuleInterop" => {
                    option.affects_semantic_diagnostics = true;
                    option.affects_emit = true;
                    option.affects_build_info = true;
                }
                "verbatimModuleSyntax" => {
                    option.affects_emit = true;
                    option.affects_semantic_diagnostics = true;
                    option.affects_build_info = true;
                }
                "erasableSyntaxOnly"
                | "exactOptionalPropertyTypes"
                | "isolatedDeclarations"
                | "noImplicitAny"
                | "noImplicitThis"
                | "noImplicitReturns"
                | "noImplicitOverride"
                | "noPropertyAccessFromIndexSignature"
                | "noUncheckedIndexedAccess"
                | "noUncheckedSideEffectImports"
                | "noUnusedLocals"
                | "noUnusedParameters" => {
                    option.affects_semantic_diagnostics = true;
                    option.affects_build_info = true;
                }
                "allowUmdGlobalAccess" => {
                    option.affects_semantic_diagnostics = true;
                    option.affects_build_info = true;
                }
                "moduleSuffixes" => {
                    option.list_preserve_falsy_values = true;
                    option.affects_module_resolution = true;
                }
                "sourceRoot" | "mapRoot" | "inlineSources" => {
                    option.affects_emit = true;
                    option.affects_build_info = true;
                }
                "emitDecoratorMetadata" => {
                    option.affects_semantic_diagnostics = true;
                    option.affects_emit = true;
                    option.affects_build_info = true;
                }
                "jsxImportSource" => {
                    option.affects_semantic_diagnostics = true;
                    option.affects_emit = true;
                    option.affects_build_info = true;
                    option.affects_module_resolution = true;
                    option.affects_source_file = true;
                }
                "reactNamespace" => {
                    option.affects_emit = true;
                    option.affects_build_info = true;
                }
                "skipDefaultLibCheck" | "skipLibCheck" => {
                    // We need to store these to determine whether `lib` files need to be rechecked
                    option.affects_build_info = true;
                }
                "noErrorTruncation" => {
                    option.affects_semantic_diagnostics = true;
                    option.affects_build_info = true;
                }
                "noFallthroughCasesInSwitch" => {
                    option.affects_bind_diagnostics = true;
                    option.affects_semantic_diagnostics = true;
                    option.affects_build_info = true;
                }
                "allowUnusedLabels" | "allowUnreachableCode" => {
                    option.affects_bind_diagnostics = true;
                    option.affects_semantic_diagnostics = true;
                    option.affects_build_info = true;
                }
                "noLib" => {
                    option.affects_program_structure = true;
                    // We are not returning a sourceFile for lib file when asked by the program,
                    // so pass --noLib to avoid reporting a file not found error.
                    option.transpile_option_value = Tristate::True;
                }
                "noResolve" => {
                    option.affects_module_resolution = true;
                    // We are not doing a full typecheck, we are not resolving the whole context,
                    // so pass --noResolve to avoid reporting missing file errors.
                    option.transpile_option_value = Tristate::True;
                }
                "strict" => {
                    // Though this affects semantic diagnostics, affectsSemanticDiagnostics is not set here
                    // The value of each strictFlag depends on own strictFlag value or this and never accessed directly.
                    // But we need to store `strict` in builf info, even though it won't be examined directly, so that the
                    // flags it controls (e.g. `strictNullChecks`) will be retrieved correctly
                    option.affects_build_info = true;
                }
                "alwaysStrict" => {
                    option.affects_source_file = true;
                    option.affects_emit = true;
                    option.affects_build_info = true;
                }
                "strictNullChecks"
                | "strictFunctionTypes"
                | "strictBindCallApply"
                | "strictPropertyInitialization"
                | "strictBuiltinIteratorReturn"
                | "useUnknownInCatchVariables"
                | "stableTypeOrdering" => {
                    option.affects_semantic_diagnostics = true;
                    option.affects_build_info = true;
                }
                "noEmitOnError" => {
                    option.affects_emit = true;
                    option.affects_build_info = true;
                    option.transpile_option_value = Tristate::Unknown;
                }
                "forceConsistentCasingInFileNames" | "maxNodeModuleJsDepth" => {
                    option.affects_module_resolution = true;
                }
                "generateCpuProfile" | "generateTrace" => {
                    option.is_file_path = true;
                }
                "project" => {
                    option.short_name = "p".to_owned();
                    option.is_file_path = true;
                }
                "locale" => {
                    option.extra_validation = crate::ExtraValidation::Locale;
                }
                "showConfig" | "listFilesOnly" | "ignoreConfig" => {
                    option.is_command_line_only = true;
                }
                "experimentalDecorators" => {
                    option.affects_emit = true;
                    option.affects_semantic_diagnostics = true;
                    option.affects_build_info = true;
                }
                "useDefineForClassFields" => {
                    option.affects_semantic_diagnostics = true;
                    option.affects_emit = true;
                    option.affects_build_info = true;
                }
                "version" => option.short_name = "v".to_owned(),
                _ => {}
            }
            if matches!(
                name,
                "noImplicitAny"
                    | "strictNullChecks"
                    | "strictFunctionTypes"
                    | "strictBindCallApply"
                    | "strictPropertyInitialization"
                    | "strictBuiltinIteratorReturn"
                    | "noImplicitThis"
                    | "useUnknownInCatchVariables"
            ) {
                option.strict_flag = true;
            }
            apply_compiler_option_metadata(&mut option);
            option
        })
        .collect();
    common_options_with_build.append(&mut compiler_options);
    common_options_with_build
});

enum MetadataDefault {
    Bool(bool),
    String(&'static str),
    Number(i32),
    Unknown,
}

struct CompilerOptionMetadata {
    name: &'static str,
    short_name: Option<&'static str>,
    category: Option<&'static str>,
    description: Option<&'static str>,
    default_value_description: Option<MetadataDefault>,
    show_in_simplified_help_view: Option<bool>,
    is_command_line_only: bool,
    is_file_path: bool,
    is_tsconfig_only: bool,
}

fn metadata_default(value: &MetadataDefault) -> DefaultValueDescription {
    match value {
        MetadataDefault::Bool(value) => DefaultValueDescription::Bool(*value),
        MetadataDefault::String(value) => DefaultValueDescription::String((*value).to_owned()),
        MetadataDefault::Number(value) => DefaultValueDescription::Number(*value),
        MetadataDefault::Unknown => DefaultValueDescription::Unknown,
    }
}

const fn metadata(
    name: &'static str,
    category: &'static str,
    description: &'static str,
    default_value_description: Option<MetadataDefault>,
) -> CompilerOptionMetadata {
    CompilerOptionMetadata {
        name,
        short_name: None,
        category: Some(category),
        description: Some(description),
        default_value_description,
        show_in_simplified_help_view: None,
        is_command_line_only: false,
        is_file_path: false,
        is_tsconfig_only: false,
    }
}

const fn command_line_metadata(
    name: &'static str,
    description: &'static str,
    show_in_simplified_help_view: bool,
    is_command_line_only: bool,
) -> CompilerOptionMetadata {
    let mut metadata = metadata(
        name,
        "Command-line Options",
        description,
        Some(MetadataDefault::Bool(false)),
    );
    metadata.show_in_simplified_help_view = Some(show_in_simplified_help_view);
    metadata.is_command_line_only = is_command_line_only;
    metadata
}

fn apply_compiler_option_metadata(option: &mut CommandLineOption) {
    let Some(metadata) = COMPILER_OPTION_METADATA
        .iter()
        .find(|metadata| metadata.name == option.name)
    else {
        return;
    };

    if let Some(short_name) = metadata.short_name {
        option.short_name = short_name.to_owned();
    }
    if let Some(show_in_simplified_help_view) = metadata.show_in_simplified_help_view {
        option.show_in_simplified_help_view = show_in_simplified_help_view;
    }
    if let Some(category) = metadata.category {
        option.category = Some(category.to_owned());
    }
    if let Some(description) = metadata.description {
        option.description = Some(description.to_owned());
    }
    if let Some(default_value_description) = &metadata.default_value_description {
        option.default_value_description = Some(metadata_default(default_value_description));
    }
    if metadata.is_command_line_only {
        option.is_command_line_only = true;
    }
    if metadata.is_file_path {
        option.is_file_path = true;
    }
    if metadata.is_tsconfig_only {
        option.is_tsconfig_only = true;
    }
}

static COMPILER_OPTION_METADATA: &[CompilerOptionMetadata] = &[
    //******* compilerOptions not common with --build *******

    // CommandLine only options
    command_line_metadata("all", "Show all compiler options.", true, false),
    CompilerOptionMetadata {
        short_name: Some("v"),
        ..command_line_metadata("version", "Print the compiler's version.", true, false)
    },
    command_line_metadata(
        "init",
        "Initializes a TypeScript project and creates a tsconfig.json file.",
        true,
        false,
    ),
    CompilerOptionMetadata {
        short_name: Some("p"),
        default_value_description: None,
        is_file_path: true,
        ..command_line_metadata(
            "project",
            "Compile the project given the path to its configuration file, or to a folder with a 'tsconfig.json'.",
            true,
            false,
        )
    },
    CompilerOptionMetadata {
        is_command_line_only: true,
        ..command_line_metadata(
            "showConfig",
            "Print the final configuration instead of building.",
            true,
            true,
        )
    },
    command_line_metadata(
        "listFilesOnly",
        "Print names of files that are part of the compilation and then stop processing.",
        false,
        true,
    ),
    command_line_metadata(
        "ignoreConfig",
        "Ignore the tsconfig found and build with commandline options and files.",
        true,
        true,
    ),
    // Basic
    // targetOptionDeclaration,
    CompilerOptionMetadata {
        short_name: Some("t"),
        show_in_simplified_help_view: Some(true),
        ..metadata(
            "target",
            "Language and Environment",
            "Set the JavaScript language version for emitted JavaScript and include compatible library declarations.",
            Some(MetadataDefault::String("ES2025")),
        )
    },
    // moduleOptionDeclaration,
    CompilerOptionMetadata {
        short_name: Some("m"),
        show_in_simplified_help_view: Some(true),
        ..metadata(
            "module",
            "Modules",
            "Specify what module code is generated.",
            Some(MetadataDefault::Unknown),
        )
    },
    CompilerOptionMetadata {
        show_in_simplified_help_view: Some(true),
        ..metadata(
            "lib",
            "Language and Environment",
            "Specify a set of bundled library declaration files that describe the target runtime environment.",
            None,
        )
    },
    // elements: &CommandLineOption{
    // 	name:                    "lib",
    // 	kind:                   CommandLineOptionTypeEnum, // libMap,
    // 	defaultValueDescription: core.TSUnknown,
    // },
    CompilerOptionMetadata {
        show_in_simplified_help_view: Some(true),
        ..metadata(
            "allowJs",
            "JavaScript Support",
            "Allow JavaScript files to be a part of your program. Use the 'checkJs' option to get errors from these files.",
            Some(MetadataDefault::String("`false`, unless `checkJs` is set")),
        )
    },
    CompilerOptionMetadata {
        show_in_simplified_help_view: Some(true),
        ..metadata(
            "checkJs",
            "JavaScript Support",
            "Enable error reporting in type-checked JavaScript files.",
            Some(MetadataDefault::Bool(false)),
        )
    },
    CompilerOptionMetadata {
        show_in_simplified_help_view: Some(true),
        ..metadata(
            "jsx",
            "Language and Environment",
            "Specify what JSX code is generated.",
            Some(MetadataDefault::Unknown),
        )
    },
    CompilerOptionMetadata {
        show_in_simplified_help_view: Some(true),
        is_file_path: true,
        ..metadata(
            "outFile",
            "Emit",
            "Specify a file that bundles all outputs into one JavaScript file. If 'declaration' is true, also designates a file that bundles all .d.ts output.",
            None,
        )
    },
    CompilerOptionMetadata {
        show_in_simplified_help_view: Some(true),
        is_file_path: true,
        ..metadata(
            "outDir",
            "Emit",
            "Specify an output folder for all emitted files.",
            None,
        )
    },
    CompilerOptionMetadata {
        is_file_path: true,
        ..metadata(
            "rootDir",
            "Modules",
            "Specify the root folder within your source files.",
            Some(MetadataDefault::String(
                "Computed from the list of input files",
            )),
        )
    },
    CompilerOptionMetadata {
        is_tsconfig_only: true,
        ..metadata(
            "composite",
            "Projects",
            "Enable constraints that allow a TypeScript project to be used with project references.",
            Some(MetadataDefault::Bool(false)),
        )
    },
    CompilerOptionMetadata {
        is_file_path: true,
        ..metadata(
            "tsBuildInfoFile",
            "Projects",
            "Specify the path to .tsbuildinfo incremental compilation file.",
            Some(MetadataDefault::String(".tsbuildinfo")),
        )
    },
    CompilerOptionMetadata {
        show_in_simplified_help_view: Some(true),
        ..metadata(
            "removeComments",
            "Emit",
            "Disable emitting comments.",
            Some(MetadataDefault::Bool(false)),
        )
    },
    metadata(
        "importHelpers",
        "Emit",
        "Allow importing helper functions from tslib once per project, instead of including them per-file.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "downlevelIteration",
        "Emit",
        "Emit more compliant, but verbose and less performant JavaScript for iteration.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "isolatedModules",
        "Interop Constraints",
        "Ensure that each file can be safely transpiled without relying on other imports.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "verbatimModuleSyntax",
        "Interop Constraints",
        "Do not transform or elide any imports or exports not marked as type-only, ensuring they are written in the output file's format based on the 'module' setting.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "isolatedDeclarations",
        "Interop Constraints",
        "Require sufficient annotation on exports so other tools can trivially generate declaration files.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "erasableSyntaxOnly",
        "Interop Constraints",
        "Do not allow runtime constructs that are not part of ECMAScript.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "libReplacement",
        "Language and Environment",
        "Enable lib replacement.",
        Some(MetadataDefault::Bool(false)),
    ),
    // Strict Type Checks
    CompilerOptionMetadata {
        show_in_simplified_help_view: Some(true),
        ..metadata(
            "strict",
            "Type Checking",
            "Enable all strict type-checking options.",
            Some(MetadataDefault::Bool(true)),
        )
    },
    metadata(
        "noImplicitAny",
        "Type Checking",
        "Enable error reporting for expressions and declarations with an implied 'any' type.",
        Some(MetadataDefault::String(
            "`true`, unless `strict` is `false`",
        )),
    ),
    metadata(
        "strictNullChecks",
        "Type Checking",
        "When type checking, take into account 'null' and 'undefined'.",
        Some(MetadataDefault::String(
            "`true`, unless `strict` is `false`",
        )),
    ),
    metadata(
        "strictFunctionTypes",
        "Type Checking",
        "When assigning functions, check to ensure parameters and the return values are subtype-compatible.",
        Some(MetadataDefault::String(
            "`true`, unless `strict` is `false`",
        )),
    ),
    metadata(
        "strictBindCallApply",
        "Type Checking",
        "Check that the arguments for 'bind', 'call', and 'apply' methods match the original function.",
        Some(MetadataDefault::String(
            "`true`, unless `strict` is `false`",
        )),
    ),
    metadata(
        "strictPropertyInitialization",
        "Type Checking",
        "Check for class properties that are declared but not set in the constructor.",
        Some(MetadataDefault::String(
            "`true`, unless `strict` is `false`",
        )),
    ),
    metadata(
        "strictBuiltinIteratorReturn",
        "Type Checking",
        "Built-in iterators are instantiated with a 'TReturn' type of 'undefined' instead of 'any'.",
        Some(MetadataDefault::String(
            "`true`, unless `strict` is `false`",
        )),
    ),
    metadata(
        "noImplicitThis",
        "Type Checking",
        "Enable error reporting when 'this' is given the type 'any'.",
        Some(MetadataDefault::String(
            "`true`, unless `strict` is `false`",
        )),
    ),
    metadata(
        "useUnknownInCatchVariables",
        "Type Checking",
        "Default catch clause variables as 'unknown' instead of 'any'.",
        Some(MetadataDefault::String(
            "`true`, unless `strict` is `false`",
        )),
    ),
    metadata(
        "alwaysStrict",
        "Type Checking",
        "Ensure 'use strict' is always emitted.",
        Some(MetadataDefault::Bool(true)),
    ),
    metadata(
        "stableTypeOrdering",
        "Type Checking",
        "Ensure types are ordered stably and deterministically across compilations.",
        Some(MetadataDefault::Bool(true)),
    ),
    // Additional Checks
    metadata(
        "noUnusedLocals",
        "Type Checking",
        "Enable error reporting when local variables aren't read.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "noUnusedParameters",
        "Type Checking",
        "Raise an error when a function parameter isn't read.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "exactOptionalPropertyTypes",
        "Type Checking",
        "Interpret optional property types as written, rather than adding 'undefined'.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "noImplicitReturns",
        "Type Checking",
        "Enable error reporting for codepaths that do not explicitly return in a function.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "noFallthroughCasesInSwitch",
        "Type Checking",
        "Enable error reporting for fallthrough cases in switch statements.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "noUncheckedIndexedAccess",
        "Type Checking",
        "Add 'undefined' to a type when accessed using an index.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "noImplicitOverride",
        "Type Checking",
        "Ensure overriding members in derived classes are marked with an override modifier.",
        Some(MetadataDefault::Bool(false)),
    ),
    CompilerOptionMetadata {
        show_in_simplified_help_view: Some(false),
        ..metadata(
            "noPropertyAccessFromIndexSignature",
            "Type Checking",
            "Enforces using indexed accessors for keys declared using an indexed type.",
            Some(MetadataDefault::Bool(false)),
        )
    },
    // Module Resolution
    //    new Map(Object.entries({
    //         // N.B. The first entry specifies the value shown in `tsc --init`
    //         node10: ModuleResolutionKind.Node10,
    //         node: ModuleResolutionKind.Node10,
    //         classic: ModuleResolutionKind.Classic,
    //         node16: ModuleResolutionKind.Node16,
    //         nodenext: ModuleResolutionKind.NodeNext,
    //         bundler: ModuleResolutionKind.Bundler,
    //     })),
    metadata(
        "moduleResolution",
        "Modules",
        "Specify how TypeScript looks up a file from a given module specifier.",
        Some(MetadataDefault::String(
            "`nodenext` if `module` is `nodenext`; `node16` if `module` is `node16` or `node18`; otherwise, `bundler`.",
        )),
    ),
    CompilerOptionMetadata {
        is_file_path: true,
        ..metadata(
            "baseUrl",
            "Modules",
            "Specify the base directory to resolve non-relative module names.",
            None,
        )
    },
    CompilerOptionMetadata {
        is_tsconfig_only: true,
        ..metadata(
            "paths",
            "Modules",
            "Specify a set of entries that re-map imports to additional lookup locations.",
            None,
        )
    },
    CompilerOptionMetadata {
        is_tsconfig_only: true,
        ..metadata(
            "rootDirs",
            "Modules",
            "Allow multiple folders to be treated as one when resolving modules.",
            Some(MetadataDefault::String(
                "Computed from the list of input files",
            )),
        )
    },
    metadata(
        "typeRoots",
        "Modules",
        "Specify multiple folders that act like './node_modules/@types'.",
        None,
    ),
    CompilerOptionMetadata {
        show_in_simplified_help_view: Some(true),
        ..metadata(
            "types",
            "Modules",
            "Specify type package names to be included without being referenced in a source file.",
            None,
        )
    },
    metadata(
        "allowSyntheticDefaultImports",
        "Interop Constraints",
        "Allow 'import x from y' when a module doesn't have a default export.",
        Some(MetadataDefault::Bool(true)),
    ),
    CompilerOptionMetadata {
        show_in_simplified_help_view: Some(true),
        ..metadata(
            "esModuleInterop",
            "Interop Constraints",
            "Emit additional JavaScript to ease support for importing CommonJS modules. This enables 'allowSyntheticDefaultImports' for type compatibility.",
            Some(MetadataDefault::Bool(true)),
        )
    },
    metadata(
        "preserveSymlinks",
        "Interop Constraints",
        "Disable resolving symlinks to their realpath. This correlates to the same flag in node.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "allowUmdGlobalAccess",
        "Modules",
        "Allow accessing UMD globals from modules.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "moduleSuffixes",
        "Modules",
        "List of file name suffixes to search when resolving a module.",
        None,
    ),
    metadata(
        "allowImportingTsExtensions",
        "Modules",
        "Allow imports to include TypeScript file extensions. Requires '--moduleResolution bundler' and either '--noEmit' or '--emitDeclarationOnly' to be set.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "rewriteRelativeImportExtensions",
        "Modules",
        "Rewrite '.ts', '.tsx', '.mts', and '.cts' file extensions in relative import paths to their JavaScript equivalent in output files.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "resolvePackageJsonExports",
        "Modules",
        "Use the package.json 'exports' field when resolving package imports.",
        Some(MetadataDefault::String(
            "`true` when 'moduleResolution' is 'node16', 'nodenext', or 'bundler'; otherwise `false`.",
        )),
    ),
    metadata(
        "resolvePackageJsonImports",
        "Modules",
        "Use the package.json 'imports' field when resolving imports.",
        Some(MetadataDefault::String(
            "`true` when 'moduleResolution' is 'node16', 'nodenext', or 'bundler'; otherwise `false`.",
        )),
    ),
    metadata(
        "customConditions",
        "Modules",
        "Conditions to set in addition to the resolver-specific defaults when resolving imports.",
        None,
    ),
    metadata(
        "noUncheckedSideEffectImports",
        "Modules",
        "Check side effect imports.",
        Some(MetadataDefault::Bool(true)),
    ),
    // Source Maps
    metadata(
        "sourceRoot",
        "Emit",
        "Specify the root path for debuggers to find the reference source code.",
        None,
    ),
    metadata(
        "mapRoot",
        "Emit",
        "Specify the location where debugger should locate map files instead of generated locations.",
        None,
    ),
    metadata(
        "inlineSources",
        "Emit",
        "Include source code in the sourcemaps inside the emitted JavaScript.",
        Some(MetadataDefault::Bool(false)),
    ),
    // Experimental
    metadata(
        "experimentalDecorators",
        "Language and Environment",
        "Enable experimental support for legacy experimental decorators.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "emitDecoratorMetadata",
        "Language and Environment",
        "Emit design-type metadata for decorated declarations in source files.",
        Some(MetadataDefault::Bool(false)),
    ),
    // Advanced
    metadata(
        "jsxFactory",
        "Language and Environment",
        "Specify the JSX factory function used when targeting React JSX emit, e.g. 'React.createElement' or 'h'.",
        Some(MetadataDefault::String("`React.createElement`")),
    ),
    metadata(
        "jsxFragmentFactory",
        "Language and Environment",
        "Specify the JSX Fragment reference used for fragments when targeting React JSX emit e.g. 'React.Fragment' or 'Fragment'.",
        Some(MetadataDefault::String("React.Fragment")),
    ),
    metadata(
        "jsxImportSource",
        "Language and Environment",
        "Specify module specifier used to import the JSX factory functions when using 'jsx: react-jsx*'.",
        Some(MetadataDefault::String("react")),
    ),
    metadata(
        "resolveJsonModule",
        "Modules",
        "Enable importing .json files.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "allowArbitraryExtensions",
        "Modules",
        "Enable importing files with any extension, provided a declaration file is present.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "reactNamespace",
        "Language and Environment",
        "Specify the object invoked for 'createElement'. This only applies when targeting 'react' JSX emit.",
        Some(MetadataDefault::String("`React`")),
    ),
    metadata(
        "skipDefaultLibCheck",
        "Completeness",
        "Skip type checking .d.ts files that are included with TypeScript.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "emitBOM",
        "Emit",
        "Emit a UTF-8 Byte Order Mark (BOM) in the beginning of output files.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "newLine",
        "Emit",
        "Set the newline character for emitting files.",
        Some(MetadataDefault::String("lf")),
    ),
    metadata(
        "noErrorTruncation",
        "Output Formatting",
        "Disable truncating types in error messages.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "noLib",
        "Language and Environment",
        "Disable including any library files, including the default lib.d.ts.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "noResolve",
        "Modules",
        "Disallow 'import's, 'require's or '<reference>'s from expanding the number of files TypeScript should add to a project.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "stripInternal",
        "Emit",
        "Disable emitting declarations that have '@internal' in their JSDoc comments.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "disableSizeLimit",
        "Editor Support",
        "Remove the 20mb cap on total source code size for JavaScript files in the TypeScript language server.",
        Some(MetadataDefault::Bool(false)),
    ),
    CompilerOptionMetadata {
        is_tsconfig_only: true,
        ..metadata(
            "disableSourceOfProjectReferenceRedirect",
            "Projects",
            "Disable preferring source files instead of declaration files when referencing composite projects.",
            Some(MetadataDefault::Bool(false)),
        )
    },
    CompilerOptionMetadata {
        is_tsconfig_only: true,
        ..metadata(
            "disableSolutionSearching",
            "Projects",
            "Opt a project out of multi-project reference checking when editing.",
            Some(MetadataDefault::Bool(false)),
        )
    },
    CompilerOptionMetadata {
        is_tsconfig_only: true,
        ..metadata(
            "disableReferencedProjectLoad",
            "Projects",
            "Reduce the number of projects loaded automatically by TypeScript.",
            Some(MetadataDefault::Bool(false)),
        )
    },
    metadata(
        "noEmitHelpers",
        "Emit",
        "Disable generating custom helper functions like '__extends' in compiled output.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "noEmitOnError",
        "Emit",
        "Disable emitting files if any type checking errors are reported.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "preserveConstEnums",
        "Emit",
        "Disable erasing 'const enum' declarations in generated code.",
        Some(MetadataDefault::Bool(false)),
    ),
    CompilerOptionMetadata {
        is_file_path: true,
        ..metadata(
            "declarationDir",
            "Emit",
            "Specify the output directory for generated declaration files.",
            None,
        )
    },
    metadata(
        "skipLibCheck",
        "Completeness",
        "Skip type checking all .d.ts files.",
        Some(MetadataDefault::Bool(false)),
    ),
    metadata(
        "allowUnusedLabels",
        "Type Checking",
        "Disable error reporting for unused labels.",
        Some(MetadataDefault::Unknown),
    ),
    metadata(
        "allowUnreachableCode",
        "Type Checking",
        "Disable error reporting for unreachable code.",
        Some(MetadataDefault::Unknown),
    ),
    metadata(
        "forceConsistentCasingInFileNames",
        "Interop Constraints",
        "Ensure that casing is correct in imports.",
        Some(MetadataDefault::Bool(true)),
    ),
    metadata(
        "maxNodeModuleJsDepth",
        "JavaScript Support",
        "Specify the maximum folder depth used for checking JavaScript files from 'node_modules'. Only applicable with 'allowJs'.",
        Some(MetadataDefault::Number(0)),
    ),
    metadata(
        "useDefineForClassFields",
        "Language and Environment",
        "Emit ECMAScript-standard-compliant class fields.",
        Some(MetadataDefault::String(
            "`true` for ES2022 and above, including ESNext.",
        )),
    ),
    // A list of plugins to load in the language service
    CompilerOptionMetadata {
        is_tsconfig_only: true,
        ..metadata(
            "plugins",
            "Editor Support",
            "Specify a list of language service plugins to include.",
            None,
        )
    },
    metadata(
        "moduleDetection",
        "Language and Environment",
        "Control what method is used to detect module-format JS files.",
        Some(MetadataDefault::String(
            "\"auto\": Treat files with imports, exports, import.meta, jsx (with jsx: react-jsx), or esm format (with module: node16+) as modules.",
        )),
    ),
    CompilerOptionMetadata {
        name: "ignoreDeprecations",
        short_name: None,
        category: None,
        description: None,
        default_value_description: Some(MetadataDefault::Unknown),
        show_in_simplified_help_view: None,
        is_command_line_only: false,
        is_file_path: false,
        is_tsconfig_only: false,
    },
];

pub fn options_declarations() -> &'static [CommandLineOption] {
    &OPTIONS_DECLARATIONS
}

pub fn options_declaration_for(name: &str) -> Option<CommandLineOption> {
    options_declarations()
        .iter()
        .find(|option| option.name == name)
        .cloned()
}

pub fn default_true_option(name: &str) -> CommandLineOption {
    CommandLineOption {
        name: name.to_owned(),
        kind: Some(CommandLineOptionKind::Boolean),
        default_value_description: Some(DefaultValueDescription::Bool(true)),
        ..CommandLineOption::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_line_option_matches_upstream_default_description_shape() {
        let option = options_declaration_for("newLine").expect("newLine option should exist");
        assert_eq!(
            option.default_value_description,
            Some(DefaultValueDescription::String("lf".to_owned()))
        );
    }
}
