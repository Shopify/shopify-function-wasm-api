use shopify_function_wasm_api_core::write::WriteResult;

#[derive(Default, Debug, PartialEq, Eq)]
pub(crate) enum State {
    #[default]
    Start,
    Object(ObjectState),
    End,
}

impl State {
    pub fn start_object(&mut self, length: usize) -> WriteResult {
        match self {
            State::Start => {
                *self = State::Object(ObjectState {
                    length,
                    num_inserted: 0,
                    parent: None,
                });
                WriteResult::Ok
            }
            State::Object(object_state) => object_state.start_object(length),
            State::End => WriteResult::ValueAlreadyWritten,
        }
    }

    pub fn write_string(&mut self) -> WriteResult {
        match self {
            State::Start => {
                *self = State::End;
                WriteResult::Ok
            }
            State::Object(object_state) => object_state.write_string(),
            State::End => WriteResult::ValueAlreadyWritten,
        }
    }

    pub fn write_non_string_scalar(&mut self) -> WriteResult {
        match self {
            State::Start => {
                *self = State::End;
                WriteResult::Ok
            }
            State::Object(object_state) => object_state.write_non_string_scalar(),
            State::End => WriteResult::ValueAlreadyWritten,
        }
    }

    pub fn finish_object(&mut self) -> WriteResult {
        match self {
            State::Object(object_state) => {
                if object_state.num_inserted != object_state.length * 2 {
                    return WriteResult::ObjectLengthError;
                }
                *self = object_state
                    .parent
                    .take()
                    .map(|parent| *parent)
                    .unwrap_or(State::End);
                WriteResult::Ok
            }
            _ => WriteResult::NotAnObject,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ObjectState {
    /// The length of the object. This is the number of key-value pairs.
    length: usize,
    /// The number of values inserted into the object. This includes keys and values,
    /// so should approach `length * 2`.
    num_inserted: usize,
    /// The parent state, if this is a nested object.
    parent: Option<Box<State>>,
}

impl ObjectState {
    fn write_string(&mut self) -> WriteResult {
        if self.num_inserted / 2 >= self.length {
            return WriteResult::ObjectLengthError;
        }
        self.num_inserted += 1;
        WriteResult::Ok
    }

    fn write_non_string_scalar(&mut self) -> WriteResult {
        self.increment_for_value()
    }

    fn increment_for_value(&mut self) -> WriteResult {
        if self.num_inserted % 2 == 0 {
            return WriteResult::ExpectedKey;
        }
        self.num_inserted += 1;
        WriteResult::Ok
    }

    fn start_object(&mut self, length: usize) -> WriteResult {
        let result = self.increment_for_value();
        if result != WriteResult::Ok {
            return result;
        }
        let mut new_object_state = ObjectState {
            length,
            num_inserted: 0,
            parent: None,
        };
        std::mem::swap(self, &mut new_object_state);
        self.parent = Some(Box::new(State::Object(new_object_state)));
        WriteResult::Ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_non_string_scalar() {
        let mut state = State::Start;
        assert_eq!(state.write_non_string_scalar(), WriteResult::Ok);
        assert_eq!(state, State::End);
        assert_eq!(
            state.write_non_string_scalar(),
            WriteResult::ValueAlreadyWritten
        );
    }

    #[test]
    fn test_write_string() {
        let mut state = State::Start;
        assert_eq!(state.write_string(), WriteResult::Ok);
        assert_eq!(state, State::End);
        assert_eq!(state.write_string(), WriteResult::ValueAlreadyWritten);
    }

    #[test]
    fn test_object() {
        let mut state = State::Start;
        assert_eq!(state.start_object(2), WriteResult::Ok);
        assert_eq!(state.write_non_string_scalar(), WriteResult::ExpectedKey);
        assert_eq!(state.write_string(), WriteResult::Ok);
        assert_eq!(state.write_string(), WriteResult::Ok);
        assert_eq!(state.write_string(), WriteResult::Ok);
        assert_eq!(state.start_object(0), WriteResult::Ok);
        assert_eq!(state.finish_object(), WriteResult::Ok);
        assert_eq!(state.write_string(), WriteResult::ObjectLengthError);
        assert_eq!(state.finish_object(), WriteResult::Ok);
        assert_eq!(state, State::End);
        assert_eq!(state.start_object(0), WriteResult::ValueAlreadyWritten);
    }
}
