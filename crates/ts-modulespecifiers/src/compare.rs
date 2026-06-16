pub fn count_path_components(path: &str) -> usize {
    let initial = if path.starts_with("./") { 2 } else { 0 };
    path[initial..].matches('/').count()
}
