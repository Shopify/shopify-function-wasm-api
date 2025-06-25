use shopify_function_wasm_api::{Context, Serialize};

/// Example demonstrating how to avoid null fields in output
/// This addresses the issue described in GitHub where the Rust output
/// was including null attributes, image, and price fields.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Original approach - includes null fields
    println!("=== Original approach (includes nulls) ===");
    let mut context1 = Context::new_with_input(serde_json::json!({}));

    let attributes: Option<String> = None;
    let image: Option<String> = None;
    let price: Option<f64> = None;
    let merchandise_id = "gid://shopify/ProductVariant/456";
    let quantity = 1;

    context1.write_object(
        |ctx| {
            ctx.write_utf8_str("merchandiseId")?;
            merchandise_id.serialize(ctx)?;

            ctx.write_utf8_str("quantity")?;
            quantity.serialize(ctx)?;

            ctx.write_utf8_str("attributes")?;
            attributes.serialize(ctx)?; // This writes null

            ctx.write_utf8_str("image")?;
            image.serialize(ctx)?; // This writes null

            ctx.write_utf8_str("price")?;
            price.serialize(ctx)?; // This writes null

            Ok(())
        },
        5,
    )?;

    let result1 = context1.finalize_output_and_return()?;
    println!(
        "Output with nulls: {}",
        serde_json::to_string_pretty(&result1)?
    );

    // New ergonomic approach - skips null fields automatically!
    println!("\n=== New approach (skips nulls) ===");
    let mut context3 = Context::new_with_input(serde_json::json!({}));

    context3.write_object_with_conditional_fields(|writer| {
        // Automatically handles key writing and field counting
        writer.field("merchandiseId", merchandise_id)?;
        writer.field("quantity", &quantity)?;
        writer.optional_field("attributes", &attributes)?; // Skipped automatically if None
        writer.optional_field("image", &image)?; // Skipped automatically if None
        writer.optional_field("price", &price)?; // Skipped automatically if None
        Ok(())
    })?;

    let result2 = context3.finalize_output_and_return()?;
    println!(
        "Output with ergonomic API: {}",
        serde_json::to_string_pretty(&result2)?
    );

    // Show size difference
    let size1 = serde_json::to_vec(&result1)?.len();
    let size2 = serde_json::to_vec(&result2)?.len();
    println!("\nSize comparison:");
    println!("With nulls: {} bytes", size1);
    println!("Without nulls (ergonomic API): {} bytes", size2);
    println!(
        "Savings: {} bytes ({:.1}%)",
        size1 - size2,
        (size1 - size2) as f32 / size1 as f32 * 100.0
    );

    Ok(())
}
