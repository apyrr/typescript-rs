use crate::{
    ImportModuleSpecifierEndingPreference, ImportModuleSpecifierPreference, ModuleSpecifierEnding,
    ModuleSpecifierGenerationHost, RelativePreferenceKind, SourceFileForSpecifierGeneration,
    UserPreferences,
};
use ts_core as core;
use ts_tspath as tspath;

struct PreferenceFileName {
    file_name: String,
    path: tspath::Path,
}

impl ts_ast::HasFileName for PreferenceFileName {
    fn file_name(&self) -> String {
        self.file_name.clone()
    }

    fn path(&self) -> tspath::Path {
        self.path.clone()
    }
}

#[derive(Clone)]
pub struct ModuleSpecifierPreferences {
    pub relative_preference: RelativePreferenceKind,
    pub allowed_endings_in_preferred_order: Vec<ModuleSpecifierEnding>,
    pub exclude_regexes: Vec<String>,
}

pub fn should_allow_importing_ts_extension(
    compiler_options: &core::CompilerOptions,
    from_file_name: &str,
) -> bool {
    // Program errors validate that `noEmit` or `emitDeclarationOnly` is also set,
    // so this function doesn't check them to avoid propagating errors.
    compiler_options.allow_importing_ts_extensions_from(from_file_name)
}

pub fn uses_extensions_on_imports(file: &impl SourceFileForSpecifierGeneration) -> bool {
    for import in file.imports() {
        let text = file.import_text(&import);
        if tspath::path_is_relative(&text)
            && !tspath::file_extension_is_one_of(
                &text,
                tspath::EXTENSIONS_NOT_SUPPORTING_EXTENSIONLESS_RESOLUTION,
            )
        {
            return tspath::has_ts_file_extension(&text) || tspath::has_js_file_extension(&text);
        }
    }
    false
}

pub fn infer_preference(
    resolution_mode: core::ResolutionMode,
    source_file: Option<&impl SourceFileForSpecifierGeneration>,
    module_resolution_is_node_next: bool,
) -> ModuleSpecifierEnding {
    let mut uses_js_extensions = false;
    if let Some(source_file) = source_file {
        let specifiers = source_file.imports();
        if specifiers.is_empty() && source_file.is_js() {
            // !!! TODO: JS support
            // specifiers = core.Map(getRequiresAtTopOfFile(sourceFile), func(d *ast.Node) *ast.Node { return d.arguments[0] })
        }
        for path in specifiers {
            let path = source_file.import_text(&path);
            if tspath::path_is_relative(&path) {
                // !!! TODO: proper resolutionMode support
                if module_resolution_is_node_next
                    && resolution_mode == core::RESOLUTION_MODE_COMMON_JS
                {
                    // We're trying to decide a preference for a CommonJS module specifier, but looking at an ESM import.
                    continue;
                }
                if tspath::file_extension_is_one_of(
                    &path,
                    tspath::EXTENSIONS_NOT_SUPPORTING_EXTENSIONLESS_RESOLUTION,
                ) {
                    // These extensions are not optional, so do not indicate a preference.
                    continue;
                }
                if tspath::has_ts_file_extension(&path) {
                    return ModuleSpecifierEnding::TsExtension;
                }
                if tspath::has_js_file_extension(&path) {
                    uses_js_extensions = true;
                }
            }
        }
    }
    if uses_js_extensions {
        ModuleSpecifierEnding::JsExtension
    } else {
        ModuleSpecifierEnding::Minimal
    }
}

pub fn get_module_specifier_ending_preference(
    pref: ImportModuleSpecifierEndingPreference,
    resolution_mode: core::ResolutionMode,
    compiler_options: &core::CompilerOptions,
    source_file: Option<&impl SourceFileForSpecifierGeneration>,
) -> ModuleSpecifierEnding {
    let module_resolution = compiler_options.get_module_resolution_kind();
    let module_resolution_is_node_next = core::ModuleResolutionKind::Node16 <= module_resolution
        && module_resolution <= core::ModuleResolutionKind::NodeNext;

    if pref == ImportModuleSpecifierEndingPreference::Js
        || resolution_mode == core::RESOLUTION_MODE_ESM && module_resolution_is_node_next
    {
        // Extensions are explicitly requested or required. Now choose between .js and .ts.
        if !should_allow_importing_ts_extension(compiler_options, "") {
            return ModuleSpecifierEnding::JsExtension;
        }
        // `allowImportingTsExtensions` is a strong signal, so use .ts unless the file
        // already uses .js extensions and no .ts extensions.
        if infer_preference(resolution_mode, source_file, module_resolution_is_node_next)
            != ModuleSpecifierEnding::JsExtension
        {
            return ModuleSpecifierEnding::TsExtension;
        }
        return ModuleSpecifierEnding::JsExtension;
    }

    if pref == ImportModuleSpecifierEndingPreference::Minimal {
        return ModuleSpecifierEnding::Minimal;
    }
    if pref == ImportModuleSpecifierEndingPreference::Index {
        return ModuleSpecifierEnding::Index;
    }

    // No preference was specified.
    // Look at imports and/or requires to guess whether .js, .ts, or extensionless imports are preferred.
    // N.B. that `Index` detection is not supported since it would require file system probing to do
    // accurately, and more importantly, literally nobody wants `Index` and its existence is a mystery.
    if !should_allow_importing_ts_extension(compiler_options, "") {
        // If .ts imports are not valid, we only need to see one .js import to go with that.
        if let Some(source_file) = source_file
            && uses_extensions_on_imports(source_file)
        {
            return ModuleSpecifierEnding::JsExtension;
        }
        return ModuleSpecifierEnding::Minimal;
    }

    infer_preference(resolution_mode, source_file, module_resolution_is_node_next)
}

pub fn get_preferred_ending(
    prefs: &UserPreferences,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    compiler_options: &core::CompilerOptions,
    importing_source_file: &impl SourceFileForSpecifierGeneration,
    old_import_specifier: &str,
    mut resolution_mode: core::ResolutionMode,
) -> ModuleSpecifierEnding {
    if !old_import_specifier.is_empty() {
        if tspath::has_js_file_extension(old_import_specifier) {
            return ModuleSpecifierEnding::JsExtension;
        }
        if old_import_specifier.ends_with("/index") {
            return ModuleSpecifierEnding::Index;
        }
    }
    if resolution_mode == core::RESOLUTION_MODE_NONE {
        resolution_mode = host.default_resolution_mode_for_file(&PreferenceFileName {
            file_name: importing_source_file.file_name(),
            path: importing_source_file.path(),
        });
    }
    get_module_specifier_ending_preference(
        prefs.import_module_specifier_ending,
        resolution_mode,
        compiler_options,
        Some(importing_source_file),
    )
}

pub fn get_allowed_endings_in_preferred_order(
    prefs: &UserPreferences,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    compiler_options: &core::CompilerOptions,
    importing_source_file: &impl SourceFileForSpecifierGeneration,
    old_import_specifier: &str,
    syntax_implied_node_format: core::ResolutionMode,
) -> Vec<ModuleSpecifierEnding> {
    let mut preferred_ending = get_preferred_ending(
        prefs,
        host,
        compiler_options,
        importing_source_file,
        old_import_specifier,
        core::RESOLUTION_MODE_NONE,
    );
    let resolution_mode = host.default_resolution_mode_for_file(&PreferenceFileName {
        file_name: importing_source_file.file_name(),
        path: importing_source_file.path(),
    });
    if resolution_mode != syntax_implied_node_format {
        preferred_ending = get_preferred_ending(
            prefs,
            host,
            compiler_options,
            importing_source_file,
            old_import_specifier,
            syntax_implied_node_format,
        );
    }
    let module_resolution = compiler_options.get_module_resolution_kind();
    let module_resolution_is_node_next = core::ModuleResolutionKind::Node16 <= module_resolution
        && module_resolution <= core::ModuleResolutionKind::NodeNext;
    let allow_importing_ts_extension =
        should_allow_importing_ts_extension(compiler_options, &importing_source_file.file_name());
    if syntax_implied_node_format == core::RESOLUTION_MODE_ESM && module_resolution_is_node_next {
        if allow_importing_ts_extension {
            return vec![
                ModuleSpecifierEnding::TsExtension,
                ModuleSpecifierEnding::JsExtension,
            ];
        }
        return vec![ModuleSpecifierEnding::JsExtension];
    }
    match preferred_ending {
        ModuleSpecifierEnding::JsExtension if allow_importing_ts_extension => vec![
            ModuleSpecifierEnding::JsExtension,
            ModuleSpecifierEnding::TsExtension,
            ModuleSpecifierEnding::Minimal,
            ModuleSpecifierEnding::Index,
        ],
        ModuleSpecifierEnding::JsExtension => vec![
            ModuleSpecifierEnding::JsExtension,
            ModuleSpecifierEnding::Minimal,
            ModuleSpecifierEnding::Index,
        ],
        ModuleSpecifierEnding::TsExtension => vec![
            ModuleSpecifierEnding::TsExtension,
            ModuleSpecifierEnding::Minimal,
            ModuleSpecifierEnding::JsExtension,
            ModuleSpecifierEnding::Index,
        ],
        ModuleSpecifierEnding::Index if allow_importing_ts_extension => vec![
            ModuleSpecifierEnding::Index,
            ModuleSpecifierEnding::Minimal,
            ModuleSpecifierEnding::TsExtension,
            ModuleSpecifierEnding::JsExtension,
        ],
        ModuleSpecifierEnding::Index => vec![
            ModuleSpecifierEnding::Index,
            ModuleSpecifierEnding::Minimal,
            ModuleSpecifierEnding::JsExtension,
        ],
        ModuleSpecifierEnding::Minimal if allow_importing_ts_extension => vec![
            ModuleSpecifierEnding::Minimal,
            ModuleSpecifierEnding::Index,
            ModuleSpecifierEnding::TsExtension,
            ModuleSpecifierEnding::JsExtension,
        ],
        ModuleSpecifierEnding::Minimal => vec![
            ModuleSpecifierEnding::Minimal,
            ModuleSpecifierEnding::Index,
            ModuleSpecifierEnding::JsExtension,
        ],
    }
}

pub fn get_module_specifier_preferences(
    prefs: &UserPreferences,
    host: &(impl ModuleSpecifierGenerationHost + ?Sized),
    compiler_options: &core::CompilerOptions,
    importing_source_file: &impl SourceFileForSpecifierGeneration,
    old_import_specifier: &str,
) -> ModuleSpecifierPreferences {
    let relative_preference = if !old_import_specifier.is_empty() {
        if tspath::is_external_module_name_relative(old_import_specifier) {
            RelativePreferenceKind::Relative
        } else {
            RelativePreferenceKind::NonRelative
        }
    } else {
        match prefs.import_module_specifier_preference {
            ImportModuleSpecifierPreference::Relative => RelativePreferenceKind::Relative,
            ImportModuleSpecifierPreference::NonRelative => RelativePreferenceKind::NonRelative,
            ImportModuleSpecifierPreference::ProjectRelative => {
                RelativePreferenceKind::ExternalNonRelative
            }
            ImportModuleSpecifierPreference::None | ImportModuleSpecifierPreference::Shortest => {
                RelativePreferenceKind::Shortest
            }
        }
    };
    let allowed_endings_in_preferred_order = get_allowed_endings_in_preferred_order(
        prefs,
        host,
        compiler_options,
        importing_source_file,
        old_import_specifier,
        host.default_resolution_mode_for_file(&PreferenceFileName {
            file_name: importing_source_file.file_name(),
            path: importing_source_file.path(),
        }),
    );
    ModuleSpecifierPreferences {
        relative_preference,
        allowed_endings_in_preferred_order,
        exclude_regexes: prefs.auto_import_specifier_exclude_regexes.clone(),
    }
}
