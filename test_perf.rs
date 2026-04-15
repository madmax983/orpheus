fn main() {
    let whole = "123";
    let fractional = "456";

    // approach 1: format
    let combined = format!("{whole}{fractional}");

    // approach 2: String::with_capacity
    let mut buffer = String::with_capacity(whole.len() + fractional.len());
    buffer.push_str(whole);
    buffer.push_str(fractional);
}
