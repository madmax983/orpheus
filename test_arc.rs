use std::sync::Arc;
fn main() {
    let a = Arc::new(5);
    let b = Arc::unwrap_or_clone(a);
}
