mod mock_generated;
mod wrapper;
#[cfg(test)]
mod wrapper_test;

pub use mock_generated::FsMock as GeneratedFsMock;
pub use wrapper::{FsMock, wrap};
