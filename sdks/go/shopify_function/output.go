package shopify_function

import (
	"errors"
	"unsafe"
)

var ErrWrite = errors.New("write error")

func checkWriteResult(result int32) error {
	if result == 0 {
		return nil
	}
	return ErrWrite
}

// OutputBool writes a boolean value to the output.
func OutputBool(value bool) error {
	var v uint32
	if value {
		v = 1
	}
	return checkWriteResult(outputNewBool(v))
}

// OutputNull writes a null value to the output.
func OutputNull() error {
	return checkWriteResult(outputNewNull())
}

// OutputI32 writes a 32-bit integer value to the output.
func OutputI32(value int32) error {
	return checkWriteResult(outputNewI32(value))
}

// OutputF64 writes a 64-bit float value to the output.
func OutputF64(value float64) error {
	return checkWriteResult(outputNewF64(value))
}

// OutputString writes a UTF-8 string value to the output.
func OutputString(value string) error {
	if len(value) == 0 {
		return checkWriteResult(outputNewUtf8Str(nil, 0))
	}
	bytes := unsafe.Slice(unsafe.StringData(value), len(value))
	return checkWriteResult(outputNewUtf8Str(&bytes[0], uint32(len(value))))
}

// OutputStringBytes writes a UTF-8 string from a byte slice to the output.
func OutputStringBytes(value []byte) error {
	if len(value) == 0 {
		return checkWriteResult(outputNewUtf8Str(nil, 0))
	}
	return checkWriteResult(outputNewUtf8Str(&value[0], uint32(len(value))))
}

// OutputInternedString writes an interned string value to the output.
func OutputInternedString(id uint32) error {
	return checkWriteResult(outputNewInternedUtf8Str(id))
}

// OutputObject begins writing an object with the given number of key-value pairs.
func OutputObject(length uint32) error {
	return checkWriteResult(outputNewObject(length))
}

// OutputFinishObject finalizes the current object being written.
func OutputFinishObject() error {
	return checkWriteResult(outputFinishObject())
}

// OutputArray begins writing an array with the given number of elements.
func OutputArray(length uint32) error {
	return checkWriteResult(outputNewArray(length))
}

// OutputFinishArray finalizes the current array being written.
func OutputFinishArray() error {
	return checkWriteResult(outputFinishArray())
}

// InternString interns a string for efficient reuse and returns its ID.
func InternString(value string) uint32 {
	if len(value) == 0 {
		return internUtf8Str(nil, 0)
	}
	bytes := unsafe.Slice(unsafe.StringData(value), len(value))
	return internUtf8Str(&bytes[0], uint32(len(value)))
}

// Log writes a log message.
func Log(message string) {
	if len(message) == 0 {
		return
	}
	bytes := unsafe.Slice(unsafe.StringData(message), len(message))
	logNewUtf8Str(&bytes[0], uint32(len(message)))
}
