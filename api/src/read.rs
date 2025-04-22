use crate::Value;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid type")]
    InvalidType,
}

pub trait Deserialize: Sized {
    fn deserialize(value: &Value) -> Result<Self, Error>;
}

impl Deserialize for Value {
    fn deserialize(value: &Value) -> Result<Self, Error> {
        Ok(*value)
    }
}

impl Deserialize for bool {
    fn deserialize(value: &Value) -> Result<Self, Error> {
        value.as_bool().ok_or(Error::InvalidType)
    }
}

impl Deserialize for i32 {
    fn deserialize(value: &Value) -> Result<Self, Error> {
        value
            .as_number()
            .and_then(|n| {
                if n.trunc() == n && n >= i32::MIN as f64 && n <= i32::MAX as f64 {
                    Some(n as i32)
                } else {
                    None
                }
            })
            .ok_or(Error::InvalidType)
    }
}

impl Deserialize for f64 {
    fn deserialize(value: &Value) -> Result<Self, Error> {
        value.as_number().ok_or(Error::InvalidType)
    }
}

impl Deserialize for String {
    fn deserialize(value: &Value) -> Result<Self, Error> {
        value.as_string().ok_or(Error::InvalidType)
    }
}

impl<T: Deserialize> Deserialize for Option<T> {
    fn deserialize(value: &Value) -> Result<Self, Error> {
        if value.is_null() {
            Ok(None)
        } else {
            Ok(Some(T::deserialize(value)?))
        }
    }
}

impl<T: Deserialize> Deserialize for Vec<T> {
    fn deserialize(value: &Value) -> Result<Self, Error> {
        if let Some(len) = value.array_len() {
            let mut vec = Vec::with_capacity(len);
            for i in 0..len {
                vec.push(T::deserialize(&value.get_at_index(i))?);
            }
            Ok(vec)
        } else {
            Err(Error::InvalidType)
        }
    }
}
