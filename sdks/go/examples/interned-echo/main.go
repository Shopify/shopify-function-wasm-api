package main

import (
	sf "github.com/Shopify/shopify-function-wasm-api/sdks/go/shopify_function"
	"math"
)

var internedFoo uint32
var internedBar uint32

func echoValue(val sf.Value) {
	switch val.Tag() {
	case sf.TagNull:
		sf.OutputNull()

	case sf.TagBool:
		b, _ := val.AsBool()
		sf.OutputBool(b)

	case sf.TagNumber:
		num, _ := val.AsNumber()
		truncated := math.Trunc(num)
		if truncated == num && num >= -2147483648.0 && num <= 2147483647.0 {
			sf.OutputI32(int32(num))
		} else {
			sf.OutputF64(num)
		}

	case sf.TagString:
		l := val.StringLen()
		buf := make([]byte, l)
		val.ReadString(buf)
		sf.OutputStringBytes(buf)

	case sf.TagObject:
		l, _ := val.ObjLen()
		sf.OutputObject(l)
		for i := uint32(0); i < l; i++ {
			key := val.GetObjKeyAtIndex(i)
			keyLen := key.StringLen()
			keyBuf := make([]byte, keyLen)
			key.ReadString(keyBuf)
			keyStr := string(keyBuf)

			if keyStr == "foo" {
				// Use interned string for key and interned obj prop for value
				sf.OutputInternedString(internedFoo)
				child := val.GetInternedObjProp(internedFoo)
				echoValue(child)
			} else if keyStr == "bar" {
				sf.OutputInternedString(internedBar)
				child := val.GetInternedObjProp(internedBar)
				echoValue(child)
			} else {
				sf.OutputStringBytes(keyBuf)
				child := val.GetAtIndex(i)
				echoValue(child)
			}
		}
		sf.OutputFinishObject()

	case sf.TagArray:
		l, _ := val.ArrayLen()
		sf.OutputArray(l)
		for i := uint32(0); i < l; i++ {
			child := val.GetAtIndex(i)
			echoValue(child)
		}
		sf.OutputFinishArray()

	default:
		sf.OutputNull()
	}
}

func main() {
	// Intern strings at startup
	internedFoo = sf.InternString("foo")
	internedBar = sf.InternString("bar")

	// Log to exercise log API
	sf.Log("interned-echo")

	input := sf.InputGet()
	echoValue(input)
}
