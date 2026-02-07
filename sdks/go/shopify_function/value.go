package shopify_function

import (
	"math"
	"unsafe"
)

// NaN-box constants for wasm32 (Val = int64)
const (
	nanMask          uint64 = 0x7FFC000000000000
	tagMask          uint64 = 0x0003C00000000000
	valueMask        uint64 = 0x00003FFFFFFFFFFF
	pointerMask      uint64 = 0x00000000FFFFFFFF
	tagShift                = 46
	valueEncSize            = 32
	maxValueLength   uint32 = 16383
)

// ValueTag represents the type of a NaN-boxed value.
type ValueTag uint8

const (
	TagNull   ValueTag = 0
	TagBool   ValueTag = 1
	TagNumber ValueTag = 2
	TagString ValueTag = 3
	TagObject ValueTag = 4
	TagArray  ValueTag = 5
	TagError  ValueTag = 15
)

// Value wraps a raw NaN-boxed int64 value from the Shopify Function API.
type Value struct {
	raw int64
}

// InputGet retrieves the root input value.
func InputGet() Value {
	return Value{raw: inputGet()}
}

// Tag returns the type tag of the value.
func (v Value) Tag() ValueTag {
	bits := uint64(v.raw)
	if (bits & nanMask) != nanMask {
		return TagNumber
	}
	return ValueTag((bits & tagMask) >> tagShift)
}

// IsNull returns true if the value is null.
func (v Value) IsNull() bool {
	return v.Tag() == TagNull
}

// AsBool returns the boolean value, or false and ok=false if not a bool.
func (v Value) AsBool() (val bool, ok bool) {
	if v.Tag() != TagBool {
		return false, false
	}
	bits := uint64(v.raw)
	return (bits & pointerMask) != 0, true
}

// AsNumber returns the float64 value, or 0 and ok=false if not a number.
func (v Value) AsNumber() (val float64, ok bool) {
	if v.Tag() != TagNumber {
		return 0, false
	}
	return math.Float64frombits(uint64(v.raw)), true
}

func (v Value) inlineLen() uint32 {
	bits := uint64(v.raw)
	return uint32((bits & valueMask) >> valueEncSize)
}

func (v Value) ptr() uint32 {
	bits := uint64(v.raw)
	return uint32(bits & pointerMask)
}

// StringLen returns the length of a string value in bytes.
func (v Value) StringLen() uint32 {
	l := v.inlineLen()
	if l < maxValueLength {
		return l
	}
	return inputGetValLen(v.raw)
}

// ReadString reads the string value into the provided buffer.
func (v Value) ReadString(buf []byte) {
	if len(buf) == 0 {
		return
	}
	inputReadUtf8Str(v.ptr(), &buf[0], uint32(len(buf)))
}

// ReadStringAlloc reads the string value and returns it as a Go string.
func (v Value) ReadStringAlloc() string {
	l := v.StringLen()
	if l == 0 {
		return ""
	}
	buf := make([]byte, l)
	v.ReadString(buf)
	return unsafe.String(&buf[0], len(buf))
}

// ObjLen returns the number of entries in the object, or 0 and ok=false if not an object.
func (v Value) ObjLen() (length uint32, ok bool) {
	if v.Tag() != TagObject {
		return 0, false
	}
	l := v.inlineLen()
	if l < maxValueLength {
		return l, true
	}
	return inputGetValLen(v.raw), true
}

// ArrayLen returns the number of elements in the array, or 0 and ok=false if not an array.
func (v Value) ArrayLen() (length uint32, ok bool) {
	if v.Tag() != TagArray {
		return 0, false
	}
	l := v.inlineLen()
	if l < maxValueLength {
		return l, true
	}
	return inputGetValLen(v.raw), true
}

// GetAtIndex returns the value at the given index (for arrays or objects).
func (v Value) GetAtIndex(index uint32) Value {
	return Value{raw: inputGetAtIndex(v.raw, index)}
}

// GetObjKeyAtIndex returns the key at the given index in an object.
func (v Value) GetObjKeyAtIndex(index uint32) Value {
	return Value{raw: inputGetObjKeyAtIndex(v.raw, index)}
}

// GetObjProp returns the value of the named property in an object.
func (v Value) GetObjProp(name string) Value {
	if len(name) == 0 {
		return Value{}
	}
	nameBytes := unsafe.Slice(unsafe.StringData(name), len(name))
	return Value{raw: inputGetObjProp(v.raw, &nameBytes[0], uint32(len(name)))}
}

// GetInternedObjProp returns the value of a property looked up by interned string ID.
func (v Value) GetInternedObjProp(id uint32) Value {
	return Value{raw: inputGetInternedObjProp(v.raw, id)}
}
