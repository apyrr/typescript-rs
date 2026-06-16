use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

use ts_ast as ast;
use ts_diagnostics::{self as diagnostics, Any};
use ts_module as module;
use ts_scanner as scanner;
use ts_tsoptions as tsoptions;
use ts_tspath as tspath;

use crate::Program;

pub(crate) type FileIncludeKind = i32;

pub(crate) const FILE_INCLUDE_KIND_IMPORT: FileIncludeKind = 0;
pub(crate) const FILE_INCLUDE_KIND_REFERENCE_FILE: FileIncludeKind = 1;
pub(crate) const FILE_INCLUDE_KIND_TYPE_REFERENCE_DIRECTIVE: FileIncludeKind = 2;
pub(crate) const FILE_INCLUDE_KIND_LIB_REFERENCE_DIRECTIVE: FileIncludeKind = 3;

pub(crate) const FILE_INCLUDE_KIND_ROOT_FILE: FileIncludeKind = 4;
pub(crate) const FILE_INCLUDE_KIND_LIB_FILE: FileIncludeKind = 5;
pub(crate) const FILE_INCLUDE_KIND_AUTOMATIC_TYPE_DIRECTIVE_FILE: FileIncludeKind = 6;

macro_rules! diagnostic_args {
    ($($arg:expr),* $(,)?) => {
        vec![$(Box::new($arg) as Any),*]
    };
}

#[derive(Clone)]
pub struct FileIncludeReason {
    pub(crate) kind: FileIncludeKind,
    pub(crate) data: FileIncludeReasonData,

    // Uses relative file name
    relative_file_name_diag: OnceLock<ast::Diagnostic>,

    // Uses file name as is
    diag: OnceLock<ast::Diagnostic>,
}

#[derive(Clone)]
pub struct ReferencedFileData {
    pub(crate) file: tspath::Path,
    pub(crate) index: isize,
    pub(crate) synthetic: Option<ast::Node>,
}

pub(crate) struct ReferenceFileLocation {
    pub(crate) file: ast::SourceFile,
    pub(crate) node: Option<ast::Node>,
    pub(crate) r#ref: Option<ast::FileReference>,
    pub(crate) package_id: module::PackageId,
    pub(crate) is_synthetic: bool,
}

impl Clone for ReferenceFileLocation {
    fn clone(&self) -> Self {
        Self {
            file: self.file.share_readonly(),
            node: self.node,
            r#ref: self.r#ref.clone(),
            package_id: self.package_id.clone(),
            is_synthetic: self.is_synthetic,
        }
    }
}

impl ReferenceFileLocation {
    pub(crate) fn text(&self) -> String {
        if let Some(node) = &self.node {
            let store = self.file.store();
            let loc = store.loc(*node);
            if !ast::node_is_synthesized(store, *node) {
                return self.file.text()[scanner::skip_trivia(self.file.text(), loc.pos() as usize)
                    ..loc.end() as usize]
                    .to_string();
            } else {
                return format!("\"{}\"", store.text(*node));
            }
        } else {
            let r#ref = self.r#ref.as_ref().unwrap();
            return self.file.text()
                [r#ref.text_range.pos() as usize..r#ref.text_range.end() as usize]
                .to_string();
        }
    }

    pub(crate) fn diagnostic_at(
        &self,
        message: &'static diagnostics::Message,
        args: Vec<Any>,
    ) -> ast::Diagnostic {
        if let Some(node) = &self.node {
            tsoptions::create_diagnostic_for_node_in_source_file(&self.file, *node, message, &args)
        } else {
            ast::new_diagnostic(
                Some(&self.file),
                self.r#ref.as_ref().unwrap().text_range,
                message,
                &args,
            )
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct AutomaticTypeDirectiveFileData {
    pub(crate) type_reference: String,
    pub(crate) package_id: module::PackageId,
}

impl Hash for AutomaticTypeDirectiveFileData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.type_reference.hash(state);
        self.package_id.name.hash(state);
        self.package_id.sub_module_name.hash(state);
        self.package_id.version.hash(state);
        self.package_id.peer_dependencies.hash(state);
    }
}

#[derive(Clone)]
pub enum FileIncludeReasonData {
    Index(usize),
    ReferencedFile(ReferencedFileData),
    AutomaticTypeDirectiveFile(AutomaticTypeDirectiveFileData),
    None,
}

impl PartialEq for ReferencedFileData {
    fn eq(&self, other: &Self) -> bool {
        self.file == other.file && self.index == other.index && self.synthetic == other.synthetic
    }
}

impl Eq for ReferencedFileData {}

impl Hash for ReferencedFileData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.file.hash(state);
        self.index.hash(state);
        self.synthetic.hash(state);
    }
}

impl PartialEq for FileIncludeReasonData {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Index(left), Self::Index(right)) => left == right,
            (Self::ReferencedFile(left), Self::ReferencedFile(right)) => left == right,
            (Self::AutomaticTypeDirectiveFile(left), Self::AutomaticTypeDirectiveFile(right)) => {
                left == right
            }
            (Self::None, Self::None) => true,
            _ => false,
        }
    }
}

impl Eq for FileIncludeReasonData {}

impl Hash for FileIncludeReasonData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Index(index) => index.hash(state),
            Self::ReferencedFile(data) => data.hash(state),
            Self::AutomaticTypeDirectiveFile(data) => data.hash(state),
            Self::None => {}
        }
    }
}

impl PartialEq for FileIncludeReason {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.data == other.data
    }
}

impl Eq for FileIncludeReason {}

impl Hash for FileIncludeReason {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
        self.data.hash(state);
    }
}

impl From<usize> for FileIncludeReasonData {
    fn from(value: usize) -> Self {
        Self::Index(value)
    }
}

impl From<()> for FileIncludeReasonData {
    fn from(_: ()) -> Self {
        Self::None
    }
}

impl From<ReferencedFileData> for FileIncludeReasonData {
    fn from(value: ReferencedFileData) -> Self {
        Self::ReferencedFile(value)
    }
}

impl From<AutomaticTypeDirectiveFileData> for FileIncludeReasonData {
    fn from(value: AutomaticTypeDirectiveFileData) -> Self {
        Self::AutomaticTypeDirectiveFile(value)
    }
}

impl FileIncludeReason {
    pub(crate) fn new(kind: FileIncludeKind, data: impl Into<FileIncludeReasonData>) -> Self {
        Self {
            kind,
            data: data.into(),
            relative_file_name_diag: OnceLock::new(),
            diag: OnceLock::new(),
        }
    }

    fn as_index(&self) -> usize {
        match self.data {
            FileIncludeReasonData::Index(index) => index,
            _ => panic!("expected index"),
        }
    }

    fn as_lib_file_index(&self) -> Option<usize> {
        match self.data {
            FileIncludeReasonData::Index(index) => Some(index),
            _ => None,
        }
    }

    pub(crate) fn is_referenced_file(&self) -> bool {
        self.kind <= FILE_INCLUDE_KIND_LIB_REFERENCE_DIRECTIVE
    }

    fn as_referenced_file_data(&self) -> &ReferencedFileData {
        match &self.data {
            FileIncludeReasonData::ReferencedFile(data) => data,
            _ => panic!("expected referencedFileData"),
        }
    }

    fn as_automatic_type_directive_file_data(&self) -> &AutomaticTypeDirectiveFileData {
        match &self.data {
            FileIncludeReasonData::AutomaticTypeDirectiveFile(data) => data,
            _ => panic!("expected automaticTypeDirectiveFileData"),
        }
    }

    pub(crate) fn get_referenced_location(&self, program: &Program) -> ReferenceFileLocation {
        let r#ref = self.as_referenced_file_data();
        let file = program.get_source_file_by_path(r#ref.file.clone()).unwrap();
        match self.kind {
            FILE_INCLUDE_KIND_IMPORT => {
                let mut specifier = None;
                let mut is_synthetic = false;
                if let Some(synthetic) = r#ref.synthetic.as_ref() {
                    specifier = Some(synthetic.clone());
                    is_synthetic = true;
                } else if r#ref.index >= 0 && (r#ref.index as usize) < file.imports().len() {
                    specifier = Some(file.imports()[r#ref.index as usize].clone());
                } else {
                    let mut aug_index = file.imports().len() as isize;
                    for imp in file.module_augmentations() {
                        if file.store().kind(*imp) == ast::Kind::StringLiteral {
                            if aug_index == r#ref.index {
                                specifier = Some(imp.clone());
                                break;
                            }
                            aug_index += 1;
                        }
                    }
                }
                let specifier = specifier.unwrap();
                let resolution = program
                    .get_resolved_module_from_module_specifier(&file, &specifier)
                    .unwrap_or_default();
                ReferenceFileLocation {
                    file,
                    node: Some(specifier),
                    r#ref: None,
                    package_id: resolution.package_id,
                    is_synthetic,
                }
            }
            FILE_INCLUDE_KIND_REFERENCE_FILE => {
                let file_ref = file.referenced_files()[r#ref.index as usize].clone();
                ReferenceFileLocation {
                    file,
                    node: None,
                    r#ref: Some(file_ref),
                    package_id: Default::default(),
                    is_synthetic: false,
                }
            }
            FILE_INCLUDE_KIND_TYPE_REFERENCE_DIRECTIVE => {
                let file_ref = file.type_reference_directives()[r#ref.index as usize].clone();
                ReferenceFileLocation {
                    file,
                    node: None,
                    r#ref: Some(file_ref),
                    package_id: Default::default(),
                    is_synthetic: false,
                }
            }
            FILE_INCLUDE_KIND_LIB_REFERENCE_DIRECTIVE => {
                let file_ref = file.lib_reference_directives()[r#ref.index as usize].clone();
                ReferenceFileLocation {
                    file,
                    node: None,
                    r#ref: Some(file_ref),
                    package_id: Default::default(),
                    is_synthetic: false,
                }
            }
            _ => panic!("unknown reason: {}", self.kind),
        }
    }

    pub(crate) fn to_diagnostic(
        &self,
        program: &Program,
        relative_file_name: bool,
    ) -> ast::Diagnostic {
        if relative_file_name {
            self.relative_file_name_diag
                .get_or_init(|| {
                    self.compute_diagnostic(program, |file_name| {
                        tspath::get_relative_path_from_directory(
                            &program.get_current_directory(),
                            file_name,
                            &program.compare_paths_options,
                        )
                    })
                })
                .clone()
        } else {
            self.diag
                .get_or_init(|| self.compute_diagnostic(program, |file_name| file_name.to_string()))
                .clone()
        }
    }
}

impl FileIncludeReason {
    pub(crate) fn to_related_info(&self, program: &Program) -> Option<ast::Diagnostic> {
        if self.is_referenced_file() {
            return self.compute_reference_file_related_info(program);
        }
        if program.opts.config.config_file.is_none() {
            return None;
        }
        let config = &program.opts.config;
        match self.kind {
            FILE_INCLUDE_KIND_ROOT_FILE => {
                let file_name = tspath::get_normalized_absolute_path(
                    &config.file_names()[self.as_index()],
                    &program.get_current_directory(),
                );
                let matched_file_spec = config.get_matched_file_spec(&file_name);
                if !matched_file_spec.is_empty() {
                    if let Some(files_node) = tsoptions::get_ts_config_prop_array_element_value(
                        &config.config_file.as_ref().unwrap().source_file,
                        "files",
                        &matched_file_spec,
                    ) {
                        return Some(tsoptions::create_diagnostic_for_node_in_source_file(
                            &config.config_file.as_ref().unwrap().source_file,
                            files_node,
                            &diagnostics::File_is_matched_by_files_list_specified_here,
                            &[],
                        ));
                    }
                } else {
                    let (matched_include_spec, is_default_include_spec) =
                        config.get_matched_include_spec(&file_name);
                    if !matched_include_spec.is_empty() {
                        if !is_default_include_spec {
                            if let Some(include_node) =
                                tsoptions::get_ts_config_prop_array_element_value(
                                    &config.config_file.as_ref().unwrap().source_file,
                                    "include",
                                    &matched_include_spec,
                                )
                            {
                                return Some(tsoptions::create_diagnostic_for_node_in_source_file(
                                    &config.config_file.as_ref().unwrap().source_file,
                                    include_node,
                                    &diagnostics::File_is_matched_by_include_pattern_specified_here,
                                    &[],
                                ));
                            }
                        }
                    }
                }
            }
            FILE_INCLUDE_KIND_AUTOMATIC_TYPE_DIRECTIVE_FILE => {
                if !program.options().uses_wildcard_types() {
                    let data = self.as_automatic_type_directive_file_data();
                    let compiler_options_syntax = program
                        .processed_files
                        .include_processor
                        .get_compiler_options_object_literal_syntax(program);
                    if let Some(types_syntax) = tsoptions::get_options_syntax_by_array_element_value(
                        config.config_file.as_ref().unwrap().source_file.store(),
                        compiler_options_syntax,
                        "types",
                        &data.type_reference,
                    ) {
                        return Some(tsoptions::create_diagnostic_for_node_in_source_file(
                            &config.config_file.as_ref().unwrap().source_file,
                            types_syntax,
                            &diagnostics::File_is_entry_point_of_type_library_specified_here,
                            &[],
                        ));
                    }
                }
            }
            FILE_INCLUDE_KIND_LIB_FILE => {
                if let Some(index) = self.as_lib_file_index() {
                    let compiler_options_syntax = program
                        .processed_files
                        .include_processor
                        .get_compiler_options_object_literal_syntax(program);
                    if let Some(lib_syntax) = tsoptions::get_options_syntax_by_array_element_value(
                        config.config_file.as_ref().unwrap().source_file.store(),
                        compiler_options_syntax,
                        "lib",
                        &program.options().lib[index],
                    ) {
                        return Some(tsoptions::create_diagnostic_for_node_in_source_file(
                            &config.config_file.as_ref().unwrap().source_file,
                            lib_syntax,
                            &diagnostics::File_is_library_specified_here,
                            &[],
                        ));
                    }
                } else if !program
                    .options()
                    .get_emit_script_target()
                    .to_string()
                    .is_empty()
                {
                    let target = program.options().get_emit_script_target().to_string();
                    let compiler_options_syntax = program
                        .processed_files
                        .include_processor
                        .get_compiler_options_object_literal_syntax(program);
                    if let Some(target_value_syntax) = tsoptions::for_each_property_assignment(
                        config.config_file.as_ref().unwrap().source_file.store(),
                        compiler_options_syntax,
                        "target",
                        tsoptions::get_callback_for_finding_property_assignment_by_value(
                            config.config_file.as_ref().unwrap().source_file.store(),
                            &target,
                        ),
                        "",
                    ) {
                        return Some(tsoptions::create_diagnostic_for_node_in_source_file(
                            &config.config_file.as_ref().unwrap().source_file,
                            target_value_syntax,
                            &diagnostics::File_is_default_library_for_target_specified_here,
                            &[],
                        ));
                    }
                }
            }
            _ => panic!("unknown reason: {}", self.kind),
        }
        None
    }

    fn compute_reference_file_related_info(&self, program: &Program) -> Option<ast::Diagnostic> {
        let reference_location = program
            .processed_files
            .include_processor
            .get_reference_location(self, program);
        if reference_location.is_synthetic {
            return None;
        }
        match self.kind {
            FILE_INCLUDE_KIND_IMPORT => Some(
                reference_location
                    .diagnostic_at(&diagnostics::File_is_included_via_import_here, Vec::new()),
            ),
            FILE_INCLUDE_KIND_REFERENCE_FILE => Some(reference_location.diagnostic_at(
                &diagnostics::File_is_included_via_reference_here,
                Vec::new(),
            )),
            FILE_INCLUDE_KIND_TYPE_REFERENCE_DIRECTIVE => Some(reference_location.diagnostic_at(
                &diagnostics::File_is_included_via_type_library_reference_here,
                Vec::new(),
            )),
            FILE_INCLUDE_KIND_LIB_REFERENCE_DIRECTIVE => Some(reference_location.diagnostic_at(
                &diagnostics::File_is_included_via_library_reference_here,
                Vec::new(),
            )),
            _ => panic!("unknown reason: {}", self.kind),
        }
    }
}

impl FileIncludeReason {
    fn compute_diagnostic(
        &self,
        program: &Program,
        to_file_name: impl Fn(&str) -> String,
    ) -> ast::Diagnostic {
        if self.is_referenced_file() {
            return self.compute_reference_file_diagnostic(program, to_file_name);
        }
        match self.kind {
            FILE_INCLUDE_KIND_ROOT_FILE => {
                if program.opts.config.config_file.is_some() {
                    let config = &program.opts.config;
                    let file_name = tspath::get_normalized_absolute_path(
                        &config.file_names()[self.as_index()],
                        &program.get_current_directory(),
                    );
                    let matched_file_spec = config.get_matched_file_spec(&file_name);
                    if !matched_file_spec.is_empty() {
                        ast::new_compiler_diagnostic(
                            &diagnostics::Part_of_files_list_in_tsconfig_json,
                            &diagnostic_args![matched_file_spec, to_file_name(&file_name)],
                        )
                    } else {
                        let (matched_include_spec, is_default_include_spec) =
                            config.get_matched_include_spec(&file_name);
                        if matched_include_spec.is_empty() {
                            ast::new_compiler_diagnostic(
                                &diagnostics::Root_file_specified_for_compilation,
                                &[],
                            )
                        } else {
                            if is_default_include_spec {
                                ast::new_compiler_diagnostic(&diagnostics::Matched_by_default_include_pattern_Asterisk_Asterisk_Slash_Asterisk, &[])
                            } else {
                                ast::new_compiler_diagnostic(
                                    &diagnostics::Matched_by_include_pattern_0_in_1,
                                    &diagnostic_args![
                                        matched_include_spec,
                                        to_file_name(&config.config_name())
                                    ],
                                )
                            }
                        }
                    }
                } else {
                    ast::new_compiler_diagnostic(
                        &diagnostics::Root_file_specified_for_compilation,
                        &[],
                    )
                }
            }
            FILE_INCLUDE_KIND_AUTOMATIC_TYPE_DIRECTIVE_FILE => {
                let data = self.as_automatic_type_directive_file_data();
                if !program.options().uses_wildcard_types() {
                    if !data.package_id.name.is_empty() {
                        ast::new_compiler_diagnostic(&diagnostics::Entry_point_of_type_library_0_specified_in_compilerOptions_with_packageId_1, &diagnostic_args![data.type_reference.clone(), data.package_id.to_string()])
                    } else {
                        ast::new_compiler_diagnostic(
                            &diagnostics::Entry_point_of_type_library_0_specified_in_compilerOptions,
                            &diagnostic_args![data.type_reference.clone()],
                        )
                    }
                } else if !data.package_id.name.is_empty() {
                    ast::new_compiler_diagnostic(
                        &diagnostics::Entry_point_for_implicit_type_library_0_with_packageId_1,
                        &diagnostic_args![data.type_reference.clone(), data.package_id.to_string()],
                    )
                } else {
                    ast::new_compiler_diagnostic(
                        &diagnostics::Entry_point_for_implicit_type_library_0,
                        &diagnostic_args![data.type_reference.clone()],
                    )
                }
            }
            FILE_INCLUDE_KIND_LIB_FILE => {
                if let Some(index) = self.as_lib_file_index() {
                    ast::new_compiler_diagnostic(
                        &diagnostics::Library_0_specified_in_compilerOptions,
                        &diagnostic_args![program.options().lib[index].clone()],
                    )
                } else if !program
                    .options()
                    .get_emit_script_target()
                    .to_string()
                    .is_empty()
                {
                    ast::new_compiler_diagnostic(
                        &diagnostics::Default_library_for_target_0,
                        &diagnostic_args![program.options().get_emit_script_target().to_string()],
                    )
                } else {
                    ast::new_compiler_diagnostic(&diagnostics::Default_library, &[])
                }
            }
            _ => panic!("unknown reason: {}", self.kind),
        }
    }

    fn compute_reference_file_diagnostic(
        &self,
        program: &Program,
        to_file_name: impl Fn(&str) -> String,
    ) -> ast::Diagnostic {
        let reference_location = program
            .processed_files
            .include_processor
            .get_reference_location(self, program);
        let reference_text = reference_location.text();
        match self.kind {
            FILE_INCLUDE_KIND_IMPORT => {
                if !reference_location.is_synthetic {
                    if !reference_location.package_id.name.is_empty() {
                        ast::new_compiler_diagnostic(
                            &diagnostics::Imported_via_0_from_file_1_with_packageId_2,
                            &diagnostic_args![
                                reference_text,
                                to_file_name(&reference_location.file.file_name()),
                                reference_location.package_id.to_string(),
                            ],
                        )
                    } else {
                        ast::new_compiler_diagnostic(
                            &diagnostics::Imported_via_0_from_file_1,
                            &diagnostic_args![
                                reference_text,
                                to_file_name(&reference_location.file.file_name()),
                            ],
                        )
                    }
                } else if program
                    .processed_files
                    .import_helpers_import_specifiers
                    .get(&reference_location.file.path())
                    .is_some_and(|specifier| {
                        reference_location.node.as_ref().is_some_and(|node| {
                            same_node_identity_or_span(
                                reference_location.file.store(),
                                specifier,
                                node,
                            )
                        })
                    })
                {
                    if !reference_location.package_id.name.is_empty() {
                        ast::new_compiler_diagnostic(&diagnostics::Imported_via_0_from_file_1_with_packageId_2_to_import_importHelpers_as_specified_in_compilerOptions, &diagnostic_args![reference_text, to_file_name(&reference_location.file.file_name()), reference_location.package_id.to_string()])
                    } else {
                        ast::new_compiler_diagnostic(&diagnostics::Imported_via_0_from_file_1_to_import_importHelpers_as_specified_in_compilerOptions, &diagnostic_args![reference_text, to_file_name(&reference_location.file.file_name())])
                    }
                } else if !reference_location.package_id.name.is_empty() {
                    ast::new_compiler_diagnostic(&diagnostics::Imported_via_0_from_file_1_with_packageId_2_to_import_jsx_and_jsxs_factory_functions, &diagnostic_args![reference_text, to_file_name(&reference_location.file.file_name()), reference_location.package_id.to_string()])
                } else {
                    ast::new_compiler_diagnostic(&diagnostics::Imported_via_0_from_file_1_to_import_jsx_and_jsxs_factory_functions, &diagnostic_args![reference_text, to_file_name(&reference_location.file.file_name())])
                }
            }
            FILE_INCLUDE_KIND_REFERENCE_FILE => ast::new_compiler_diagnostic(
                &diagnostics::Referenced_via_0_from_file_1,
                &diagnostic_args![
                    reference_text,
                    to_file_name(&reference_location.file.file_name())
                ],
            ),
            FILE_INCLUDE_KIND_TYPE_REFERENCE_DIRECTIVE => {
                if !reference_location.package_id.name.is_empty() {
                    ast::new_compiler_diagnostic(
                        &diagnostics::Type_library_referenced_via_0_from_file_1_with_packageId_2,
                        &diagnostic_args![
                            reference_text,
                            to_file_name(&reference_location.file.file_name()),
                            reference_location.package_id.to_string()
                        ],
                    )
                } else {
                    ast::new_compiler_diagnostic(
                        &diagnostics::Type_library_referenced_via_0_from_file_1,
                        &diagnostic_args![
                            reference_text,
                            to_file_name(&reference_location.file.file_name())
                        ],
                    )
                }
            }
            FILE_INCLUDE_KIND_LIB_REFERENCE_DIRECTIVE => ast::new_compiler_diagnostic(
                &diagnostics::Library_referenced_via_0_from_file_1,
                &diagnostic_args![
                    reference_text,
                    to_file_name(&reference_location.file.file_name())
                ],
            ),
            _ => panic!("unknown reason: {}", self.kind),
        }
    }
}

fn same_node_identity_or_span(store: &ast::AstStore, left: &ast::Node, right: &ast::Node) -> bool {
    left == right
        || (store.kind(*left) == store.kind(*right)
            && store.loc(*left) == store.loc(*right)
            && store.text(*left) == store.text(*right))
}
