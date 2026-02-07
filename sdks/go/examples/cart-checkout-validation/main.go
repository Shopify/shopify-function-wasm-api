package main

import (
	sf "github.com/Shopify/shopify-function-wasm-api/sdks/go/shopify_function"
)

func collectErrors(cart sf.Value) bool {
	if _, ok := cart.ObjLen(); !ok {
		return false
	}

	lines := cart.GetObjProp("lines")
	linesLen, ok := lines.ArrayLen()
	if !ok {
		return false
	}

	for i := uint32(0); i < linesLen; i++ {
		line := lines.GetAtIndex(i)
		if _, ok := line.ObjLen(); ok {
			quantity := line.GetObjProp("quantity")
			if q, ok := quantity.AsNumber(); ok {
				if q > 1.0 {
					return true
				}
			}
		}
	}

	return false
}

func main() {
	input := sf.InputGet()
	cart := input.GetObjProp("cart")
	hasError := collectErrors(cart)

	// {"errors": [...]}
	sf.OutputObject(1)
	sf.OutputString("errors")

	if hasError {
		sf.OutputArray(1)
		// {"localizedMessage": "...", "target": "$.cart"}
		sf.OutputObject(2)
		sf.OutputString("localizedMessage")
		sf.OutputString("Not possible to order more than one of each")
		sf.OutputString("target")
		sf.OutputString("$.cart")
		sf.OutputFinishObject()
		sf.OutputFinishArray()
	} else {
		sf.OutputArray(0)
		sf.OutputFinishArray()
	}

	sf.OutputFinishObject()
}
