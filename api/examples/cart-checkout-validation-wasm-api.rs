use shopify_function_wasm_api::{Context, Value};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let mut context = Context::new();

    let input = context.input_get()?;
    let cart = input.get_obj_prop("cart");

    let errors = collect_errors(&cart);

    context.write_object(
        |ctx| {
            ctx.write_utf8_str("errors")?;
            ctx.write_array(
                |array_ctx| {
                    for error in &errors {
                        array_ctx.write_object(
                            |error_ctx| {
                                error_ctx.write_utf8_str("localizedMessage")?;
                                error_ctx.write_utf8_str(&error)?;

                                error_ctx.write_utf8_str("target")?;
                                error_ctx.write_utf8_str("$.cart")?;

                                Ok(())
                            },
                            2,
                        )?;
                    }
                    Ok(())
                },
                errors.len(),
            )?;

            Ok(())
        },
        1,
    )?;

    context.finalize_output()?;

    Ok(())
}

// Helper function to collect errors for lines with quantity > 1
fn collect_errors(cart: &Value) -> Vec<String> {
    let mut errors = Vec::new();

    if !cart.is_obj() {
        return errors;
    }

    let lines = cart.get_obj_prop("lines");

    if !lines.is_array() {
        return errors;
    }

    if let Some(lines_len) = lines.array_len() {
        for i in 0..lines_len {
            let line = lines.get_at_index(i);
            if line.is_obj() {
                let quantity = line.get_obj_prop("quantity");
                if let Some(q) = quantity.as_number() {
                    if q > 1.0 {
                        errors.push(String::from("Not possible to order more than one of each"));
                        break;
                    }
                }
            }
        }
    }

    errors
}
