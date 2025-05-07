use std::error::Error;
use std::io::{self, Read, Write};

fn main() -> Result<(), Box<dyn Error>> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;

    let input: serde_json::Value = serde_json::from_str(&buffer)?;

    let errors = if let Some(cart) = input.get("cart") {
        collect_errors(cart)
    } else {
        Vec::new()
    };

    let mut output = serde_json::Map::new();
    let errors_array = errors
        .into_iter()
        .map(|msg| {
            let mut error_obj = serde_json::Map::new();
            error_obj.insert(
                "localizedMessage".to_string(),
                serde_json::Value::String(msg),
            );
            error_obj.insert(
                "target".to_string(),
                serde_json::Value::String("$.cart".to_string()),
            );
            serde_json::Value::Object(error_obj)
        })
        .collect();

    output.insert("errors".to_string(), serde_json::Value::Array(errors_array));

    let output_bytes = rmp_serde::to_vec(&serde_json::Value::Object(output))?;
    io::stdout().write_all(&output_bytes)?;

    Ok(())
}

fn collect_errors(cart: &serde_json::Value) -> Vec<String> {
    let mut errors = Vec::new();

    if !cart.is_object() {
        return errors;
    }

    let lines = match cart.get("lines") {
        Some(lines) if lines.is_array() => lines.as_array().unwrap(),
        _ => return errors,
    };

    for line in lines {
        if let Some(quantity) = line.get("quantity").and_then(|q| q.as_f64()) {
            if quantity > 1.0 {
                errors.push(String::from("Not possible to order more than one of each"));
                break;
            }
        }
    }

    errors
}
