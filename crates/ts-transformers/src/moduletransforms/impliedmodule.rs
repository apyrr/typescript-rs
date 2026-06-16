use ts_core::ModuleKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImpliedModuleTransform {
    DeclarationFileNoop,
    CommonJs,
    EsModule,
}

pub fn implied_module_transform_for_file(
    is_declaration_file: bool,
    emit_module_format: ModuleKind,
) -> ImpliedModuleTransform {
    if is_declaration_file {
        return ImpliedModuleTransform::DeclarationFileNoop;
    }

    if emit_module_format >= ModuleKind::ES2015 {
        ImpliedModuleTransform::EsModule
    } else {
        ImpliedModuleTransform::CommonJs
    }
}
