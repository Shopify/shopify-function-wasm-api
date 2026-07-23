use shopify_function_wasm_api_core::write::WriteResult;

#[derive(Default, Debug, PartialEq, Eq)]
pub(crate) enum State {
    #[default]
    Start,
    Object(ObjectState),
    Array(ArrayState),
    Shape(ShapeState),
    End,
}

impl State {
    pub fn start_object(
        &mut self,
        length: usize,
        parent_state_stack: &mut Vec<State>,
    ) -> WriteResult {
        let new_state = Self::Object(ObjectState {
            length,
            num_inserted: 0,
        });
        match self {
            State::Start => {
                *self = new_state;
                WriteResult::Ok
            }
            State::Object(object_state) => {
                let result = object_state.write_non_string_value();
                if result != WriteResult::Ok {
                    return result;
                }
                self.swap_and_push(new_state, parent_state_stack);
                WriteResult::Ok
            }
            State::Array(array_state) => {
                let result = array_state.write_value();
                if result != WriteResult::Ok {
                    return result;
                }
                self.swap_and_push(new_state, parent_state_stack);
                WriteResult::Ok
            }
            State::Shape(shape_state) => {
                let result = shape_state.write_value();
                if result != WriteResult::Ok {
                    return result;
                }
                self.swap_and_push(new_state, parent_state_stack);
                WriteResult::Ok
            }
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
            State::Array(array_state) => array_state.write_value(),
            State::Shape(shape_state) => shape_state.write_value(),
            State::End => WriteResult::ValueAlreadyWritten,
        }
    }

    pub fn write_non_string_scalar(&mut self) -> WriteResult {
        match self {
            State::Start => {
                *self = State::End;
                WriteResult::Ok
            }
            State::Object(object_state) => object_state.write_non_string_value(),
            State::Array(array_state) => array_state.write_value(),
            State::Shape(shape_state) => shape_state.write_value(),
            State::End => WriteResult::ValueAlreadyWritten,
        }
    }

    pub fn finish_object(&mut self, parent_state_stack: &mut Vec<State>) -> WriteResult {
        match self {
            State::Object(object_state) => {
                if object_state.num_inserted != object_state.length * 2 {
                    return WriteResult::ObjectLengthError;
                }
                *self = parent_state_stack.pop().unwrap_or(State::End);
                WriteResult::Ok
            }
            _ => WriteResult::NotAnObject,
        }
    }

    pub fn start_array(
        &mut self,
        length: usize,
        parent_state_stack: &mut Vec<State>,
    ) -> WriteResult {
        let new_state = Self::Array(ArrayState {
            length,
            num_inserted: 0,
        });
        match self {
            State::Start => {
                *self = new_state;
                WriteResult::Ok
            }
            State::Object(object_state) => {
                let result = object_state.write_non_string_value();
                if result != WriteResult::Ok {
                    return result;
                }
                self.swap_and_push(new_state, parent_state_stack);
                WriteResult::Ok
            }
            State::Array(array_state) => {
                let result = array_state.write_value();
                if result != WriteResult::Ok {
                    return result;
                }
                self.swap_and_push(new_state, parent_state_stack);
                WriteResult::Ok
            }
            State::Shape(shape_state) => {
                let result = shape_state.write_value();
                if result != WriteResult::Ok {
                    return result;
                }
                self.swap_and_push(new_state, parent_state_stack);
                WriteResult::Ok
            }
            State::End => WriteResult::ValueAlreadyWritten,
        }
    }

    pub fn finish_array(&mut self, parent_state_stack: &mut Vec<State>) -> WriteResult {
        match self {
            State::Array(array_state) => {
                if array_state.num_inserted != array_state.length {
                    return WriteResult::ArrayLengthError;
                }
                *self = parent_state_stack.pop().unwrap_or(State::End);
                WriteResult::Ok
            }
            _ => WriteResult::NotAnArray,
        }
    }

    pub fn start_shape(
        &mut self,
        key_count: usize,
        parent_state_stack: &mut Vec<State>,
    ) -> WriteResult {
        let new_state = Self::Shape(ShapeState {
            key_count,
            num_inserted: 0,
        });
        match self {
            State::Start => {
                *self = new_state;
                WriteResult::Ok
            }
            State::Object(object_state) => {
                let result = object_state.write_non_string_value();
                if result != WriteResult::Ok {
                    return result;
                }
                self.swap_and_push(new_state, parent_state_stack);
                WriteResult::Ok
            }
            State::Array(array_state) => {
                let result = array_state.write_value();
                if result != WriteResult::Ok {
                    return result;
                }
                self.swap_and_push(new_state, parent_state_stack);
                WriteResult::Ok
            }
            State::Shape(shape_state) => {
                let result = shape_state.write_value();
                if result != WriteResult::Ok {
                    return result;
                }
                self.swap_and_push(new_state, parent_state_stack);
                WriteResult::Ok
            }
            State::End => WriteResult::ValueAlreadyWritten,
        }
    }

    pub fn finish_shape(&mut self, parent_state_stack: &mut Vec<State>) -> WriteResult {
        match self {
            State::Shape(shape_state) => {
                if shape_state.num_inserted != shape_state.key_count {
                    return WriteResult::ShapeLengthError;
                }
                *self = parent_state_stack.pop().unwrap_or(State::End);
                WriteResult::Ok
            }
            _ => WriteResult::NotAShape,
        }
    }

    fn swap_and_push(&mut self, new_state: State, parent_state_stack: &mut Vec<State>) {
        let mut new_state = new_state;
        std::mem::swap(self, &mut new_state);
        parent_state_stack.push(new_state);
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ObjectState {
    /// The length of the object. This is the number of key-value pairs.
    length: usize,
    /// The number of values inserted into the object. This includes keys and values,
    /// so should approach `length * 2`.
    num_inserted: usize,
}

impl ObjectState {
    fn write_string(&mut self) -> WriteResult {
        if self.num_inserted / 2 >= self.length {
            return WriteResult::ObjectLengthError;
        }
        self.num_inserted += 1;
        WriteResult::Ok
    }

    fn write_non_string_value(&mut self) -> WriteResult {
        if self.num_inserted.is_multiple_of(2) {
            return WriteResult::ExpectedKey;
        }
        self.num_inserted += 1;
        WriteResult::Ok
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ArrayState {
    length: usize,
    num_inserted: usize,
}

impl ArrayState {
    fn write_value(&mut self) -> WriteResult {
        if self.num_inserted >= self.length {
            return WriteResult::ArrayLengthError;
        }
        self.num_inserted += 1;
        WriteResult::Ok
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ShapeState {
    pub(crate) key_count: usize,
    pub(crate) num_inserted: usize,
}

impl ShapeState {
    fn write_value(&mut self) -> WriteResult {
        if self.num_inserted >= self.key_count {
            return WriteResult::ShapeLengthError;
        }
        self.num_inserted += 1;
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
        let mut parent_state_stack = Vec::new();
        assert_eq!(
            state.start_object(3, &mut parent_state_stack),
            WriteResult::Ok
        );
        assert_eq!(state.write_non_string_scalar(), WriteResult::ExpectedKey);
        assert_eq!(state.write_string(), WriteResult::Ok);
        assert_eq!(state.write_string(), WriteResult::Ok);
        assert_eq!(state.write_string(), WriteResult::Ok);
        assert_eq!(
            state.start_object(0, &mut parent_state_stack),
            WriteResult::Ok
        );
        assert_eq!(
            state.finish_object(&mut parent_state_stack),
            WriteResult::Ok
        );
        assert_eq!(state.write_string(), WriteResult::Ok);
        assert_eq!(
            state.start_array(0, &mut parent_state_stack),
            WriteResult::Ok
        );
        assert_eq!(state.finish_array(&mut parent_state_stack), WriteResult::Ok);
        assert_eq!(state.write_string(), WriteResult::ObjectLengthError);
        assert_eq!(
            state.finish_object(&mut parent_state_stack),
            WriteResult::Ok
        );
        assert_eq!(state, State::End);
        assert_eq!(
            state.start_object(0, &mut parent_state_stack),
            WriteResult::ValueAlreadyWritten
        );
        assert_eq!(parent_state_stack, vec![]);
    }

    #[test]
    fn test_array() {
        let mut state = State::Start;
        let mut parent_state_stack = Vec::new();
        assert_eq!(
            state.start_array(3, &mut parent_state_stack),
            WriteResult::Ok
        );
        assert_eq!(state.write_non_string_scalar(), WriteResult::Ok);
        assert_eq!(
            state.finish_array(&mut parent_state_stack),
            WriteResult::ArrayLengthError
        );
        assert_eq!(
            state.start_array(0, &mut parent_state_stack),
            WriteResult::Ok
        );
        assert_eq!(state.finish_array(&mut parent_state_stack), WriteResult::Ok);
        assert_eq!(
            state.start_object(0, &mut parent_state_stack),
            WriteResult::Ok
        );
        assert_eq!(
            state.finish_object(&mut parent_state_stack),
            WriteResult::Ok
        );
        assert_eq!(state.finish_array(&mut parent_state_stack), WriteResult::Ok);
        assert_eq!(state, State::End);
        assert_eq!(
            state.start_array(0, &mut parent_state_stack),
            WriteResult::ValueAlreadyWritten
        );
        assert_eq!(parent_state_stack, vec![]);
    }

    #[test]
    fn test_shape_counts_scalars_and_nested_values() {
        let mut state = State::Start;
        let mut parent_state_stack = Vec::new();
        assert_eq!(
            state.start_shape(4, &mut parent_state_stack),
            WriteResult::Ok
        );
        assert_eq!(state.write_string(), WriteResult::Ok);

        assert_eq!(
            state.start_object(0, &mut parent_state_stack),
            WriteResult::Ok
        );
        assert_eq!(
            state.finish_object(&mut parent_state_stack),
            WriteResult::Ok
        );

        assert_eq!(
            state.start_array(0, &mut parent_state_stack),
            WriteResult::Ok
        );
        assert_eq!(state.finish_array(&mut parent_state_stack), WriteResult::Ok);

        assert_eq!(
            state.start_shape(0, &mut parent_state_stack),
            WriteResult::Ok
        );
        assert_eq!(state.finish_shape(&mut parent_state_stack), WriteResult::Ok);
        assert_eq!(
            state.write_non_string_scalar(),
            WriteResult::ShapeLengthError
        );
        assert_eq!(state.finish_shape(&mut parent_state_stack), WriteResult::Ok);
        assert_eq!(state, State::End);
        assert_eq!(
            state.finish_shape(&mut parent_state_stack),
            WriteResult::NotAShape
        );
    }

    #[test]
    fn test_shape_length_error_leaves_shape_open() {
        let mut state = State::Start;
        let mut parent_state_stack = Vec::new();
        assert_eq!(
            state.start_shape(1, &mut parent_state_stack),
            WriteResult::Ok
        );
        assert_eq!(
            state.finish_shape(&mut parent_state_stack),
            WriteResult::ShapeLengthError
        );
        assert_eq!(state.write_non_string_scalar(), WriteResult::Ok);
        assert_eq!(state.finish_shape(&mut parent_state_stack), WriteResult::Ok);
    }

    #[test]
    fn test_shape_can_be_nested_in_objects_and_arrays() {
        let mut state = State::Start;
        let mut parent_state_stack = Vec::new();
        assert_eq!(
            state.start_object(1, &mut parent_state_stack),
            WriteResult::Ok
        );
        assert_eq!(state.write_string(), WriteResult::Ok);
        assert_eq!(
            state.start_shape(0, &mut parent_state_stack),
            WriteResult::Ok
        );
        assert_eq!(state.finish_shape(&mut parent_state_stack), WriteResult::Ok);
        assert_eq!(
            state.finish_object(&mut parent_state_stack),
            WriteResult::Ok
        );

        let mut state = State::Start;
        assert_eq!(
            state.start_array(1, &mut parent_state_stack),
            WriteResult::Ok
        );
        assert_eq!(
            state.start_shape(0, &mut parent_state_stack),
            WriteResult::Ok
        );
        assert_eq!(state.finish_shape(&mut parent_state_stack), WriteResult::Ok);
        assert_eq!(state.finish_array(&mut parent_state_stack), WriteResult::Ok);
    }
}
