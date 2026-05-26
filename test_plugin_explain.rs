use orpheus_lang::{eval_module, ReplMode};
fn main() {
    let module = eval_module("p = vst(\"test\")", ReplMode::Loose).unwrap();
    let p = module.get("p").unwrap();
    // println!("{}", p.as_plugin_pattern().unwrap().explain("p"));
}
