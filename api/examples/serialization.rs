use std::{collections::HashMap, error::Error};

use shopify_function_wasm_api::{write::Error as WriteError, write::Serialize, Context};

struct MyData {
    my_string: String,
    my_i32: i32,
    my_f64: f64,
    my_bool: bool,
    my_vec: Vec<i32>,
    my_hash_map: HashMap<String, i32>,
    my_option: Option<String>,
}

impl Serialize for MyData {
    fn serialize(&self, context: &mut Context) -> Result<(), WriteError> {
        context.write_object(
            |context| {
                context.write_utf8_str("my_string")?;
                self.my_string.serialize(context)?;
                context.write_utf8_str("my_i32")?;
                self.my_i32.serialize(context)?;
                context.write_utf8_str("my_f64")?;
                self.my_f64.serialize(context)?;
                context.write_utf8_str("my_bool")?;
                self.my_bool.serialize(context)?;
                context.write_utf8_str("my_vec")?;
                self.my_vec.serialize(context)?;
                context.write_utf8_str("my_hash_map")?;
                self.my_hash_map.serialize(context)?;
                context.write_utf8_str("my_option")?;
                self.my_option.serialize(context)?;

                Ok(())
            },
            7,
        )?;
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let my_data = MyData {
        my_string: "Hello, world!".to_string(),
        my_i32: 42,
        my_f64: 1.23,
        my_bool: true,
        my_vec: vec![1, 2, 3],
        my_hash_map: HashMap::from([("foo".to_string(), 1), ("bar".to_string(), 2)]),
        my_option: None,
    };

    let mut context = Context::new();

    my_data.serialize(&mut context)?;
    context.finalize_output()?;

    Ok(())
}
