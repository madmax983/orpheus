fn main() {
    let digits = "123.456";
    let (whole, fractional) = digits.split_once('.').unwrap();

    let whole_num = whole.parse::<i128>().unwrap_or(0);
    let fractional_num = fractional.parse::<i128>().unwrap_or(0);
    let scale = 10_i128.pow(fractional.len() as u32);
    let result = whole_num * scale + fractional_num;
    println!("{}", result);

    let digits = ".456";
    let (whole, fractional) = digits.split_once('.').unwrap();
    let whole_num = whole.parse::<i128>().unwrap_or(0);
    let fractional_num = fractional.parse::<i128>().unwrap_or(0);
    let scale = 10_i128.pow(fractional.len() as u32);
    let result = whole_num * scale + fractional_num;
    println!("{}", result);

    let digits = "123.";
    let (whole, fractional) = digits.split_once('.').unwrap();
    let whole_num = whole.parse::<i128>().unwrap_or(0);
    let fractional_num = fractional.parse::<i128>().unwrap_or(0);
    let scale = 10_i128.pow(fractional.len() as u32);
    let result = whole_num * scale + fractional_num;
    println!("{}", result);

    let digits = "0.005";
    let (whole, fractional) = digits.split_once('.').unwrap();
    let whole_num = whole.parse::<i128>().unwrap_or(0);
    let fractional_num = fractional.parse::<i128>().unwrap_or(0);
    let scale = 10_i128.pow(fractional.len() as u32);
    let result = whole_num * scale + fractional_num;
    println!("{}", result);

    let digits = "123.005";
    let (whole, fractional) = digits.split_once('.').unwrap();
    let whole_num = whole.parse::<i128>().unwrap_or(0);
    let fractional_num = fractional.parse::<i128>().unwrap_or(0);
    let scale = 10_i128.pow(fractional.len() as u32);
    let result = whole_num * scale + fractional_num;
    println!("{}", result);
}
