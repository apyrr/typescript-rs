use std::sync::Arc;

use ts_core::TextPos;

pub trait Source {
    fn text(&self) -> String;
    fn file_name(&self) -> String;
    fn ecma_line_map(&self) -> Arc<[TextPos]>;
}
