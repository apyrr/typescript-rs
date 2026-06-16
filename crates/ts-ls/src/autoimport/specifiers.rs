use ts_collections::SetOptionExt;
use ts_modulespecifiers as modulespecifiers;

use crate::autoimport::{Export, View};

impl View<'_> {
    pub fn get_module_specifier(
        &self,
        export: &Export,
        user_preferences: modulespecifiers::UserPreferences,
    ) -> (String, modulespecifiers::ResultKind) {
        if modulespecifiers::path_is_bare_specifier(export.module_id()) {
            let specifier = export.module_id().to_owned();
            if modulespecifiers::is_excluded_by_regex(
                &specifier,
                &user_preferences.auto_import_specifier_exclude_regexes,
            ) {
                return (String::new(), modulespecifiers::ResultKind::None);
            }
            return (specifier, modulespecifiers::ResultKind::Ambient);
        }

        let Some(registry) = self.registry.as_ref() else {
            return (String::new(), modulespecifiers::ResultKind::None);
        };

        if !export.package_name.is_empty() {
            if let Some(entrypoints) = registry.entrypoints.get(&export.path) {
                let Some(program) = self.program else {
                    return (String::new(), modulespecifiers::ResultKind::None);
                };
                let Some(importing_file) = self.importing_file.as_ref() else {
                    return (String::new(), modulespecifiers::ResultKind::None);
                };
                for entrypoint in entrypoints {
                    if entrypoint
                        .include_conditions
                        .iter()
                        .all(|condition| self.conditions.as_ref().has(condition))
                        && !entrypoint
                            .exclude_conditions
                            .iter()
                            .any(|condition| self.conditions.as_ref().has(condition))
                    {
                        let specifier = modulespecifiers::process_entrypoint_ending(
                            entrypoint,
                            user_preferences.clone(),
                            program,
                            program.compiler_options(),
                            importing_file,
                            &self.allowed_endings,
                        );

                        if !modulespecifiers::is_excluded_by_regex(
                            &specifier,
                            &user_preferences.auto_import_specifier_exclude_regexes,
                        ) {
                            return (specifier, modulespecifiers::ResultKind::NodeModules);
                        }
                    }
                }
                return (String::new(), modulespecifiers::ResultKind::None);
            }
        }

        let Some(importing_file) = self.importing_file.as_ref() else {
            return (String::new(), modulespecifiers::ResultKind::None);
        };
        if let Some(cache) = registry.specifier_cache.get(&importing_file.path()) {
            if export.package_name.is_empty() {
                let (specifier, ok) = cache.load(&export.path);
                if ok {
                    if let Some(specifier) = specifier {
                        if specifier.is_empty() {
                            return (String::new(), modulespecifiers::ResultKind::None);
                        }
                        return (specifier, modulespecifiers::ResultKind::Relative);
                    }
                    return (String::new(), modulespecifiers::ResultKind::None);
                }
            }
        }

        let Some(program) = self.program else {
            return (String::new(), modulespecifiers::ResultKind::None);
        };
        let (specifiers, kind) = modulespecifiers::get_module_specifiers_for_file_with_info(
            importing_file,
            &export.module_file_name,
            program.compiler_options(),
            program,
            user_preferences,
            modulespecifiers::ModuleSpecifierOptions::default(),
            true,
        );
        if let Some(cache) = registry.specifier_cache.get(&importing_file.path()) {
            // !!! unsure when this could return multiple specifiers combined with the
            //     new node_modules code. Possibly with local symlinks, which should be
            //     very rare.
            for specifier in specifiers {
                if specifier.contains("/node_modules/") {
                    continue;
                }
                cache.store(export.path.clone(), Some(specifier.clone()));
                return (specifier, kind);
            }
            cache.store(export.path.clone(), Some(String::new()));
        }
        (String::new(), modulespecifiers::ResultKind::None)
    }
}
