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

fn passthrough(
    ctx: js::Context,
    _this: js::Object,
    mut arguments: Vec<js::Value>,
) -> Result<js::Value, js::String> {
    if arguments.len() >= 1 {
        Ok(arguments.remove(0))
    } else {
        Ok(ctx.undefined())
    }
}

#[runtime::main]
async fn main() {
    let ctx_group = js::ContextGroup::new();
    let ctx = ctx_group.create_global_context();
    let global = ctx.global_object();

    // let fn_name = &js::String::new("log").unwrap();
    // let fn_obj = global.make_function_with_callback(fn_name, log).to_js_value();
    // global.set_property(fn_name, fn_obj);
    ctx.add_function("log", log);
    ctx.add_function("passthrough", passthrough);

    let script = &js::String::new("log(\"it works\", 42 * 124123.21)").unwrap();
    match ctx.evaluate_script(script).await {
        Ok(v) => println!("Success!"),
        Err(e) => eprintln!("{:?}", &e),
    };

    let script = &js::String::new("passthrough(42)").unwrap();
    match ctx.evaluate_script(script).await {
        Ok(v) => println!("Result: {}, {:?}", v.to_string(), &v),
        Err(e) => eprintln!("{:?}", &e),
    };
}
