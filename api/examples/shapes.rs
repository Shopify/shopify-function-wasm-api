use shopify_function_wasm_api::Context;
use std::error::Error;

#[cfg_attr(target_family = "wasm", export_name = "_start")]
fn main() {
    run().unwrap()
}

fn run() -> Result<(), Box<dyn Error>> {
    shopify_function_wasm_api::init_panic_handler();
    let mut context = Context::new();

    let input = context.input_get()?;
    let count = input.get_obj_prop("count").as_number().unwrap_or(0.0) as usize;
    let point_shape = context.define_shape(&["x", "y"])?;
    context.write_array(
        |ctx| {
            for x in 0..count {
                ctx.write_shaped_object(point_shape, |ctx| {
                    ctx.write_i32(x as i32)?;
                    ctx.write_i32((x * 2) as i32)
                })?;
            }
            Ok(())
        },
        count,
    )?;

    Ok(())
}
