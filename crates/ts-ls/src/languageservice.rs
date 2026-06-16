use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use ts_ast as ast;
use ts_compiler as compiler;
use ts_core as core;
use ts_lsproto as lsproto;
use ts_lsproto::DocumentUriExt;
use ts_sourcemap as sourcemap;
use ts_tspath as tspath;
use ts_vfs::vfsmatch;

use crate::autoimport;
use crate::host::Host;
use crate::lsconv;
use crate::lsutil;

pub struct LanguageService<'a> {
    pub(crate) project_path: tspath::Path,
    pub(crate) host: Option<Box<dyn Host>>,
    pub(crate) active_config: lsutil::UserPreferences,
    pub(crate) program: Option<Arc<compiler::Program>>,
    pub(crate) converters: lsconv::Converters,
    pub(crate) document_position_mappers: HashMap<String, sourcemap::DocumentPositionMapper>,
    marker: PhantomData<&'a ()>,
}

impl Default for LanguageService<'_> {
    fn default() -> Self {
        Self {
            project_path: tspath::Path::default(),
            host: None,
            active_config: lsutil::UserPreferences::default(),
            program: None,
            converters: lsconv::new_converters(lsproto::PositionEncodingKind::UTF16, |_| {
                lsconv::compute_lsp_line_starts("")
            }),
            document_position_mappers: HashMap::new(),
            marker: PhantomData,
        }
    }
}

pub fn new_language_service(
    project_path: tspath::Path,
    program: Arc<compiler::Program>,
    host: Box<dyn Host>,
    active_file: &str,
) -> LanguageService<'static> {
    let converters = host.converters();
    let active_config = host.get_preferences(active_file);
    LanguageService {
        project_path,
        host: Some(host),
        program: Some(program),
        converters,
        active_config,
        document_position_mappers: HashMap::new(),
        marker: PhantomData,
    }
}

impl LanguageService<'_> {
    pub fn to_path(&self, file_name: &str) -> tspath::Path {
        tspath::to_path(
            file_name,
            &self.program.as_ref().unwrap().get_current_directory(),
            self.use_case_sensitive_file_names(),
        )
    }

    pub(crate) fn get_program(&self) -> &compiler::Program {
        self.program.as_deref().unwrap()
    }

    pub fn user_preferences(&self) -> lsutil::UserPreferences {
        self.active_config.clone()
    }

    pub fn format_options(&self) -> lsutil::FormatCodeSettings {
        self.active_config.format_code_settings.clone()
    }

    pub(crate) fn try_get_program_and_file(
        &self,
        file_name: &str,
    ) -> (&compiler::Program, Option<&ast::SourceFile>) {
        let program = self.get_program();
        let file = program.get_source_file_ref(file_name);
        (program, file)
    }

    pub(crate) fn get_program_and_file(
        &self,
        document_uri: lsproto::DocumentUri,
    ) -> (&compiler::Program, &ast::SourceFile) {
        let file_name = document_uri.file_name();
        let (program, file) = self.try_get_program_and_file(&file_name);
        if file.is_none() {
            panic!("file not found: {file_name}");
        }
        (program, file.unwrap())
    }

    pub fn get_document_position_mapper(
        &mut self,
        file_name: &str,
    ) -> Option<&sourcemap::DocumentPositionMapper> {
        if !self.document_position_mappers.contains_key(file_name) {
            if let Some(mapper) = sourcemap::get_document_position_mapper(self, file_name) {
                self.document_position_mappers
                    .insert(file_name.to_string(), mapper);
            }
        }
        self.document_position_mappers.get(file_name)
    }

    pub fn read_file(&self, file_name: &str) -> (String, bool) {
        self.host.as_ref().unwrap().read_file(file_name)
    }

    pub fn use_case_sensitive_file_names(&self) -> bool {
        self.host.as_ref().unwrap().use_case_sensitive_file_names()
    }

    pub fn get_ecma_line_info(&self, file_name: &str) -> Option<sourcemap::ECMALineInfo> {
        self.host.as_ref().unwrap().get_ecma_line_info(file_name)
    }

    // getPreparedAutoImportView returns an auto-import view for the given file if the registry is prepared
    // to provide up-to-date auto-imports for it. If not, it returns ErrNeedsAutoImports.
    // If auto-imports are disabled via user preferences, it returns (nil, nil).
    pub(crate) fn get_prepared_auto_import_view(
        &self,
        from_file: &ast::SourceFile,
    ) -> Result<Option<autoimport::View<'_>>, core::Error> {
        if self
            .user_preferences()
            .include_completions_for_module_exports
            .is_false()
        {
            return Ok(None);
        }
        let registry = self.host.as_ref().unwrap().auto_import_registry().unwrap();
        if !registry.is_prepared_for_importing_file(
            &from_file.file_name(),
            self.project_path.clone(),
            self.user_preferences(),
        ) {
            return Err(core::Error::new(crate::completions::ERR_NEEDS_AUTO_IMPORTS));
        }

        let view = autoimport::new_view(
            registry,
            from_file,
            self.project_path.clone(),
            self.get_program(),
            self.user_preferences().module_specifier_preferences(),
        );
        Ok(Some(view))
    }

    // getCurrentAutoImportView returns an auto-import view for the given file, based on the current state
    // of the auto-import registry, which may or may not be up-to-date.
    pub(crate) fn get_current_auto_import_view(
        &self,
        from_file: &ast::SourceFile,
    ) -> autoimport::View<'_> {
        autoimport::new_view(
            self.host.as_ref().unwrap().auto_import_registry().unwrap(),
            from_file,
            self.project_path.clone(),
            self.get_program(),
            self.user_preferences().module_specifier_preferences(),
        )
    }

    // Used for module specifier completions.
    pub fn directory_exists(&self, path: &str) -> bool {
        self.host.as_ref().unwrap().directory_exists(path)
    }

    // Used for module specifier completions.
    pub fn read_directory(
        &self,
        path: &str,
        extensions: &[String],
        includes: &[String],
    ) -> Vec<String> {
        self.host.as_ref().unwrap().read_directory(
            &self.get_program().get_current_directory(),
            path,
            extensions,
            &[], /*excludes*/
            includes,
            vfsmatch::UNLIMITED_DEPTH,
        )
    }

    pub fn get_directories(&self, path: &str) -> Vec<String> {
        self.host.as_ref().unwrap().get_directories(path)
    }
}

impl sourcemap::Host for LanguageService<'_> {
    fn use_case_sensitive_file_names(&self) -> bool {
        LanguageService::use_case_sensitive_file_names(self)
    }

    fn get_ecma_line_info(&self, file_name: &str) -> Option<sourcemap::ECMALineInfo> {
        LanguageService::get_ecma_line_info(self, file_name)
    }

    fn read_file(&self, file_name: &str) -> Option<String> {
        let (text, ok) = LanguageService::read_file(self, file_name);
        ok.then_some(text)
    }
}
