fn main() {
    let digits = "123.";
    let (whole, fractional) = digits.split_once('.').unwrap();
    let whole_num = whole.parse::<i128>().unwrap_or(0);
    let fractional_num = fractional.parse::<i128>().unwrap_or(0);
    let scale = 10_i128.pow(fractional.len() as u32);
    let result = whole_num * scale + fractional_num;
    println!("{}", result);
    let combined = [whole, fractional].concat();
    let combined_res = combined.parse::<i128>().unwrap_or(0);
    println!("{}", combined_res);
}
