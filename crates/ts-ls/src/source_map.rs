use ts_core as core;
use ts_lsproto as lsproto;
use ts_outputpaths as outputpaths;
use ts_sourcemap as sourcemap;
use ts_tspath as tspath;

use crate::LanguageService;
use crate::lsconv;

impl LanguageService<'_> {
    pub fn get_mapped_location(
        &self,
        file_name: &str,
        file_range: core::TextRange,
    ) -> lsproto::Location {
        let start_pos = self.try_get_source_position(file_name, file_range.pos());
        let Some(start_pos) = start_pos else {
            let script = self.get_script(file_name).expect("script expected");
            let lsp_range = self.create_lsp_range_from_range(file_range, &script);
            return lsproto::Location {
                uri: lsconv::file_name_to_document_uri(file_name),
                range: lsp_range,
            };
        };

        let mut end_pos = self.try_get_source_position(file_name, file_range.end());
        if end_pos.as_ref().is_none_or(|end_pos| {
            end_pos.file_name != start_pos.file_name || end_pos.pos < start_pos.pos
        }) {
            // When end doesn't map, maps to a different source file (e.g. in a .d.ts with a
            // multi-source source map from --outFile compilation), or maps to a position before
            // start (non-monotonic source map mappings), approximate the end position.
            end_pos = Some(sourcemap::DocumentPosition {
                file_name: start_pos.file_name.clone(),
                pos: start_pos.pos + file_range.len(),
            });
        }

        let end_pos = end_pos.unwrap();
        let new_range = core::new_text_range(start_pos.pos, end_pos.pos);
        let script = self
            .get_script(&start_pos.file_name)
            .expect("script expected");
        let lsp_range = self.create_lsp_range_from_range(new_range, &script);
        lsproto::Location {
            uri: lsconv::file_name_to_document_uri(&start_pos.file_name),
            range: lsp_range,
        }
    }

    pub fn get_script(&self, file_name: &str) -> Option<Script> {
        let (text, ok) = self.host.as_ref().unwrap().read_file(file_name);
        if !ok {
            return None;
        }
        Some(Script {
            file_name: file_name.to_string(),
            text,
        })
    }

    pub fn try_get_source_position(
        &self,
        file_name: &str,
        position: core::TextPos,
    ) -> Option<sourcemap::DocumentPosition> {
        let new_pos = self.try_get_source_position_worker(file_name, position);
        if let Some(new_pos) = &new_pos {
            if !self.read_file(&new_pos.file_name).1 {
                return None;
            }
        }
        new_pos
    }

    pub fn try_get_source_position_worker(
        &self,
        file_name: &str,
        position: core::TextPos,
    ) -> Option<sourcemap::DocumentPosition> {
        if !tspath::is_declaration_file_name(file_name) {
            return None;
        }

        let position_mapper = self
            .document_position_mappers
            .get(file_name)
            .cloned()
            .or_else(|| sourcemap::get_document_position_mapper(self, file_name));
        let Some(position_mapper) = position_mapper else {
            return None;
        };
        let document_pos = position_mapper.get_source_position(&sourcemap::DocumentPosition {
            file_name: file_name.to_string(),
            pos: position,
        })?;
        if let Some(new_pos) =
            self.try_get_source_position_worker(&document_pos.file_name, document_pos.pos)
        {
            return Some(new_pos);
        }
        Some(document_pos)
    }

    pub fn try_get_generated_position(
        &self,
        file_name: &str,
        position: core::TextPos,
    ) -> Option<sourcemap::DocumentPosition> {
        let new_pos = self.try_get_generated_position_worker(file_name, position);
        if let Some(new_pos) = &new_pos {
            if !self.read_file(&new_pos.file_name).1 {
                return None;
            }
        }
        new_pos
    }

    pub fn try_get_generated_position_worker(
        &self,
        file_name: &str,
        position: core::TextPos,
    ) -> Option<sourcemap::DocumentPosition> {
        if tspath::is_declaration_file_name(file_name) {
            return None;
        }

        let Some(program) = self.program.as_ref() else {
            return None;
        };
        if program.get_source_file_ref(file_name).is_none() {
            return None;
        }

        let path = self.to_path(file_name);
        // If this is source file of project reference source (instead of redirect) there is no generated position
        if program.is_source_from_project_reference(path) {
            return None;
        }

        let declaration_file_name = outputpaths::get_output_declaration_file_name_worker(
            file_name,
            program.options(),
            &**program,
        );
        let position_mapper = self
            .document_position_mappers
            .get(&declaration_file_name)
            .cloned()
            .or_else(|| sourcemap::get_document_position_mapper(self, &declaration_file_name));
        let Some(position_mapper) = position_mapper else {
            return None;
        };
        let document_pos =
            position_mapper.get_generated_position(&sourcemap::DocumentPosition {
                file_name: file_name.to_string(),
                pos: position,
            })?;
        if let Some(new_pos) =
            self.try_get_generated_position_worker(&document_pos.file_name, document_pos.pos)
        {
            return Some(new_pos);
        }
        Some(document_pos)
    }
}

#[derive(Clone, Debug)]
pub struct Script {
    pub file_name: String,
    pub text: String,
}

impl Script {
    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

impl lsconv::Script for Script {
    fn file_name(&self) -> &str {
        &self.file_name
    }

    fn text(&self) -> &str {
        &self.text
    }
}
