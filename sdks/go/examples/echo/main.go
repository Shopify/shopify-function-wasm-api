package main

import (
	sf "github.com/Shopify/shopify-function-wasm-api/sdks/go/shopify_function"
	"math"
)

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
			// Write key
			key := val.GetObjKeyAtIndex(i)
			keyLen := key.StringLen()
			keyBuf := make([]byte, keyLen)
			key.ReadString(keyBuf)
			sf.OutputStringBytes(keyBuf)

			// Write value
			child := val.GetAtIndex(i)
			echoValue(child)
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
	input := sf.InputGet()
	echoValue(input)
}
