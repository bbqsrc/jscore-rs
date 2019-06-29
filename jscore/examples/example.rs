#![feature(async_await)]

use jscore::types as js;
use jscore::types::ContextType;
use std::convert::TryFrom;

fn log(
    ctx: js::Context,
    _this: js::Object,
    arguments: Vec<js::Value>,
) -> Result<js::Value, js::String> {
    let v = std::string::String::try_from(&arguments[0]).unwrap();
    let n = f64::try_from(&arguments[1]).unwrap();
    println!("Hello world, {}, with a number: {}!", v, n);
    Ok(ctx.undefined())
}

#[runtime::main]
async fn main() {
    let ctx_group = js::ContextGroup::new();
    let ctx = ctx_group.create_global_context();
    let global = ctx.global_object();
    let fn_name = &js::String::new("log").unwrap();
    let fn_obj = global.make_function_with_callback(fn_name, log);
    global.set_property(fn_name, *fn_obj);

    let script = &js::String::new("log(\"it works\", 42)").unwrap();
    ctx.evaluate_script(script).await;
}
