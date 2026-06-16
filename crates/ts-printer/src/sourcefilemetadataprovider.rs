use ts_ast as ast;
use ts_tspath as tspath;

pub type SourceFileMetaData = ast::SourceFileMetaData;

pub trait SourceFileMetaDataProvider {
    fn get_source_file_meta_data(&self, path: tspath::Path) -> Option<SourceFileMetaData>;
}
