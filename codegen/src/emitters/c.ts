/**
 * C code emitter.
 * Generates C source code from SchemaModel and parsed queries.
 *
 * C has no generics/comptime, so we must generate:
 * - Output structs with explicit serialize functions per type
 * - @oneOf unions as struct { enum tag; union data; }
 * - Per-query Input types with accessor functions
 * - All string interning is static variables in accessor functions
 */

import {
  SchemaModel,
  InputObjectType,
  EnumType,
  TypeRef,
  isNullable,
  getNamedType,
  isScalar,
  isBuiltinScalar,
} from "../schema-model.js";
import { QueryFieldSelection, InlineFragmentSelection } from "../parser.js";
import { camelToSnake } from "./zig.js";

export interface CEmitterOptions {
  enumsAsStr: string[];
}

interface TargetQuery {
  targetName: string;
  graphqlTargetName: string;
  selections: QueryFieldSelection[];
}

export function emitC(
  schema: SchemaModel,
  targets: TargetQuery[],
  options: CEmitterOptions
): { header: string; source: string } {
  const h: string[] = [];
  const c: string[] = [];

  // Header preamble
  h.push("#ifndef SCHEMA_H");
  h.push("#define SCHEMA_H");
  h.push("");
  h.push('#include "shopify_function.h"');
  h.push('#include "shopify_function_value.h"');
  h.push('#include "sf_lib.h"');
  h.push("");

  // Source preamble
  c.push('#include "schema.h"');
  c.push("");

  const inputTypeOrder = getInputTypeOrder(schema);

  // Forward declarations
  h.push("// === Forward declarations ===");
  for (const typeName of inputTypeOrder) {
    const inputType = schema.inputTypes.get(typeName)!;
    if (inputType.isOneOf) {
      h.push(`typedef struct ${typeName} ${typeName};`);
      h.push(`typedef enum ${typeName}Tag ${typeName}Tag;`);
    } else {
      h.push(`typedef struct ${typeName} ${typeName};`);
    }
  }
  h.push("");

  // Emit enums
  for (const [name, enumType] of schema.enumTypes) {
    if (options.enumsAsStr.includes(name)) continue;
    h.push(...emitCEnum(enumType));
    h.push("");
    c.push(...emitCEnumImpl(enumType));
    c.push("");
  }

  // Emit output types
  h.push("// === Output types ===");
  h.push("");
  for (const typeName of inputTypeOrder) {
    const inputType = schema.inputTypes.get(typeName)!;
    if (inputType.isOneOf) {
      h.push(...emitCOneOfStruct(inputType, schema, options));
    } else {
      h.push(...emitCOutputStruct(inputType, schema, options));
    }
    h.push("");
  }

  // Emit serialize function declarations
  h.push("// === Serialization ===");
  for (const typeName of inputTypeOrder) {
    h.push(`void serialize_${typeName}(const ${typeName}* value);`);
  }
  h.push("");

  // Emit serialize function implementations
  c.push("// === Serialization ===");
  c.push("");
  for (const typeName of inputTypeOrder) {
    const inputType = schema.inputTypes.get(typeName)!;
    if (inputType.isOneOf) {
      c.push(...emitCOneOfSerialize(inputType, schema, options));
    } else {
      c.push(...emitCStructSerialize(inputType, schema, options));
    }
    c.push("");
  }

  // Emit per-target input types
  h.push("// === Input types (per-query, lazy accessors) ===");
  h.push("");
  for (const target of targets) {
    h.push(...emitCTargetInput(target, schema, options));
    h.push("");
    c.push(...emitCTargetInputImpl(target, schema, options));
    c.push("");
  }

  h.push("#endif // SCHEMA_H");

  return { header: h.join("\n"), source: c.join("\n") };
}

function getInputTypeOrder(schema: SchemaModel): string[] {
  const visited = new Set<string>();
  const order: string[] = [];
  function visit(typeName: string) {
    if (visited.has(typeName)) return;
    visited.add(typeName);
    const inputType = schema.inputTypes.get(typeName);
    if (!inputType) return;
    for (const field of inputType.fields) {
      const depName = getNamedType(field.type);
      if (schema.inputTypes.has(depName)) visit(depName);
    }
    order.push(typeName);
  }
  for (const typeName of schema.inputTypes.keys()) visit(typeName);
  return order;
}

function scalarToCType(namedType: string, schema: SchemaModel, options: CEmitterOptions): string {
  switch (namedType) {
    case "String": case "ID": case "Date": case "DateTime":
    case "DateTimeWithoutTimezone": case "TimeWithoutTimezone":
    case "Decimal": case "Handle": case "URL":
      return "sf_str_t";
    case "Int": return "int32_t";
    case "Float": return "double";
    case "Boolean": return "int";
    case "Void": return "void";
    default:
      if (options.enumsAsStr.includes(namedType)) return "sf_str_t";
      if (schema.enumTypes.has(namedType)) return `${namedType}`;
      return "sf_str_t";
  }
}

function typeRefToC(typeRef: TypeRef, schema: SchemaModel, options: CEmitterOptions): string {
  switch (typeRef.kind) {
    case "Named": return scalarToCType(typeRef.name!, schema, options);
    case "NonNull": return typeRefToC(typeRef.ofType!, schema, options);
    case "List": {
      const inner = typeRefToC(typeRef.ofType!, schema, options);
      // For lists we use a struct with pointer + count
      return inner; // handled specially in struct emission
    }
  }
}

function isListType(typeRef: TypeRef): boolean {
  if (typeRef.kind === "List") return true;
  if (typeRef.kind === "NonNull") return isListType(typeRef.ofType!);
  return false;
}

function getListInnerType(typeRef: TypeRef): TypeRef {
  if (typeRef.kind === "NonNull") return getListInnerType(typeRef.ofType!);
  if (typeRef.kind === "List") return typeRef.ofType!;
  return typeRef;
}

// --- Output struct emission ---

function emitCOutputStruct(
  inputType: InputObjectType, schema: SchemaModel, options: CEmitterOptions
): string[] {
  const lines: string[] = [];
  lines.push(`struct ${inputType.name} {`);
  for (const field of inputType.fields) {
    const nullable = isNullable(field.type);
    const isList = isListType(field.type);
    if (isList) {
      const innerType = getListInnerType(field.type);
      const namedType = getNamedType(innerType);
      const cType = isInputType(namedType, schema)
        ? `const ${namedType}*` : `const ${scalarToCType(namedType, schema, options)}*`;
      // Use a struct-in-struct for the array
      lines.push(`    struct { ${cType.replace('*', '')}* items; size_t count; } ${field.name};`);
    } else {
      const namedType = getNamedType(field.type);
      const cType = isInputType(namedType, schema) ? namedType : scalarToCType(namedType, schema, options);
      if (nullable) {
        lines.push(`    int has_${field.name};`);
      }
      lines.push(`    ${cType} ${field.name};`);
    }
  }
  lines.push("};");
  return lines;
}

function emitCOneOfStruct(
  inputType: InputObjectType, schema: SchemaModel, options: CEmitterOptions
): string[] {
  const lines: string[] = [];
  // Tag enum
  lines.push(`enum ${inputType.name}Tag {`);
  for (const field of inputType.fields) {
    lines.push(`    ${inputType.name}Tag_${field.name},`);
  }
  lines.push("};");
  lines.push("");
  // Union struct
  lines.push(`struct ${inputType.name} {`);
  lines.push(`    ${inputType.name}Tag tag;`);
  lines.push("    union {");
  for (const field of inputType.fields) {
    const namedType = getNamedType(field.type);
    const cType = isInputType(namedType, schema) ? namedType : scalarToCType(namedType, schema, options);
    lines.push(`        ${cType} ${field.name};`);
  }
  lines.push("    } data;");
  lines.push("};");
  return lines;
}

// --- Serialization ---

function emitCStructSerialize(
  inputType: InputObjectType, schema: SchemaModel, options: CEmitterOptions
): string[] {
  const lines: string[] = [];
  lines.push(`void serialize_${inputType.name}(const ${inputType.name}* value) {`);

  // All fields are always serialized (null optionals become null values)
  lines.push(`    shopify_function_output_new_object(${inputType.fields.length});`);

  for (const field of inputType.fields) {
    const nullable = isNullable(field.type) && !isListType(field.type);
    lines.push(`    shopify_function_output_new_utf8_str((const uint8_t*)"${field.name}", ${field.name.length});`);
    if (nullable) {
      lines.push(`    if (value->has_${field.name}) {`);
      lines.push(...emitCFieldSerialize(field, schema, options, "        "));
      lines.push("    } else {");
      lines.push("        shopify_function_output_new_null();");
      lines.push("    }");
    } else {
      lines.push(...emitCFieldSerialize(field, schema, options, "    "));
    }
  }
  lines.push("    shopify_function_output_finish_object();");
  lines.push("}");
  return lines;
}

function emitCFieldSerialize(
  field: { name: string; type: TypeRef },
  schema: SchemaModel,
  options: CEmitterOptions,
  indent: string
): string[] {
  const lines: string[] = [];
  const isList = isListType(field.type);
  const namedType = getNamedType(field.type);

  if (isList) {
    lines.push(`${indent}shopify_function_output_new_array(value->${field.name}.count);`);
    lines.push(`${indent}for (size_t i = 0; i < value->${field.name}.count; i++) {`);
    if (isInputType(namedType, schema)) {
      lines.push(`${indent}    serialize_${namedType}(&value->${field.name}.items[i]);`);
    } else {
      lines.push(...emitCScalarSerialize(`value->${field.name}.items[i]`, namedType, schema, options, indent + "    "));
    }
    lines.push(`${indent}}`);
    lines.push(`${indent}shopify_function_output_finish_array();`);
  } else if (isInputType(namedType, schema)) {
    lines.push(`${indent}serialize_${namedType}(&value->${field.name});`);
  } else {
    lines.push(...emitCScalarSerialize(`value->${field.name}`, namedType, schema, options, indent));
  }
  return lines;
}

function emitCScalarSerialize(
  expr: string, namedType: string, schema: SchemaModel, options: CEmitterOptions, indent: string
): string[] {
  const lines: string[] = [];
  switch (namedType) {
    case "String": case "ID": case "Date": case "DateTime":
    case "DateTimeWithoutTimezone": case "TimeWithoutTimezone":
    case "Decimal": case "Handle": case "URL":
      lines.push(`${indent}shopify_function_output_new_utf8_str((const uint8_t*)${expr}.ptr, ${expr}.len);`);
      break;
    case "Int":
      lines.push(`${indent}shopify_function_output_new_i32(${expr});`);
      break;
    case "Float":
      lines.push(`${indent}shopify_function_output_new_f64(${expr});`);
      break;
    case "Boolean":
      lines.push(`${indent}shopify_function_output_new_bool(${expr});`);
      break;
    default:
      if (options.enumsAsStr.includes(namedType)) {
        lines.push(`${indent}shopify_function_output_new_utf8_str((const uint8_t*)${expr}.ptr, ${expr}.len);`);
      } else if (schema.enumTypes.has(namedType)) {
        lines.push(`${indent}shopify_function_output_new_utf8_str((const uint8_t*)${namedType}_to_str(${expr}), ${namedType}_to_str_len(${expr}));`);
      } else {
        lines.push(`${indent}shopify_function_output_new_utf8_str((const uint8_t*)${expr}.ptr, ${expr}.len);`);
      }
  }
  return lines;
}

function emitCOneOfSerialize(
  inputType: InputObjectType, schema: SchemaModel, options: CEmitterOptions
): string[] {
  const lines: string[] = [];
  lines.push(`void serialize_${inputType.name}(const ${inputType.name}* value) {`);
  lines.push("    shopify_function_output_new_object(1);");
  lines.push("    switch (value->tag) {");
  for (const field of inputType.fields) {
    const namedType = getNamedType(field.type);
    lines.push(`    case ${inputType.name}Tag_${field.name}:`);
    lines.push(`        shopify_function_output_new_utf8_str((const uint8_t*)"${field.name}", ${field.name.length});`);
    if (isInputType(namedType, schema)) {
      lines.push(`        serialize_${namedType}(&value->data.${field.name});`);
    } else {
      lines.push(...emitCScalarSerialize(`value->data.${field.name}`, namedType, schema, options, "        "));
    }
    lines.push("        break;");
  }
  lines.push("    }");
  lines.push("    shopify_function_output_finish_object();");
  lines.push("}");
  return lines;
}

// --- Enum emission ---

function emitCEnum(enumType: EnumType): string[] {
  const lines: string[] = [];
  lines.push(`typedef enum ${enumType.name} {`);
  for (const value of enumType.values) {
    lines.push(`    ${enumType.name}_${value},`);
  }
  lines.push(`    ${enumType.name}_Other,`);
  lines.push(`} ${enumType.name};`);
  lines.push("");
  lines.push(`${enumType.name} ${enumType.name}_from_str(const char* s, size_t len);`);
  lines.push(`const char* ${enumType.name}_to_str(${enumType.name} value);`);
  lines.push(`size_t ${enumType.name}_to_str_len(${enumType.name} value);`);
  return lines;
}

function emitCEnumImpl(enumType: EnumType): string[] {
  const lines: string[] = [];
  // from_str
  lines.push(`${enumType.name} ${enumType.name}_from_str(const char* s, size_t len) {`);
  for (const value of enumType.values) {
    lines.push(`    if (len == ${value.length} && sf_str_eq(s, "${value}", ${value.length})) return ${enumType.name}_${value};`);
  }
  lines.push(`    return ${enumType.name}_Other;`);
  lines.push("}");
  lines.push("");
  // to_str
  lines.push(`const char* ${enumType.name}_to_str(${enumType.name} value) {`);
  lines.push("    switch (value) {");
  for (const value of enumType.values) {
    lines.push(`    case ${enumType.name}_${value}: return "${value}";`);
  }
  lines.push(`    default: return "";`);
  lines.push("    }");
  lines.push("}");
  lines.push("");
  // to_str_len
  lines.push(`size_t ${enumType.name}_to_str_len(${enumType.name} value) {`);
  lines.push("    switch (value) {");
  for (const value of enumType.values) {
    lines.push(`    case ${enumType.name}_${value}: return ${value.length};`);
  }
  lines.push("    default: return 0;");
  lines.push("    }");
  lines.push("}");
  return lines;
}

// --- Input accessor emission ---

/** Collect all nested wrapper type names needed (recursive) */
function collectNestedTypes(
  selections: QueryFieldSelection[],
  prefix: string,
  result: { typeName: string; field: QueryFieldSelection; parentPrefix: string }[]
): void {
  for (const field of selections) {
    const hasSubSelections = field.selections.length > 0;
    const hasInlineFragments = (field.inlineFragments?.length ?? 0) > 0;
    const isList = isListType(field.schemaType);

    if (hasInlineFragments) {
      // Union type: collect variant types
      const unionPrefix = `${prefix}_${field.name}`;
      // Push the union field itself (for tag enum + wrapper generation)
      result.push({ typeName: unionPrefix, field, parentPrefix: prefix });
      // Collect nested types within each variant
      for (const fragment of field.inlineFragments!) {
        const variantPrefix = `${unionPrefix}_${fragment.typeName}`;
        // Create a synthetic field for the variant
        const variantField: QueryFieldSelection = {
          name: fragment.typeName,
          schemaType: field.schemaType,
          selections: fragment.selections,
        };
        result.push({ typeName: variantPrefix, field: variantField, parentPrefix: unionPrefix });
        collectNestedTypes(fragment.selections, variantPrefix, result);
      }
    } else if (hasSubSelections) {
      if (isList) {
        const itemTypeName = `${prefix}_${field.name}_Item`;
        result.push({ typeName: itemTypeName, field, parentPrefix: prefix });
        collectNestedTypes(field.selections, itemTypeName, result);
      } else {
        const nestedTypeName = `${prefix}_${field.name}`;
        result.push({ typeName: nestedTypeName, field, parentPrefix: prefix });
        collectNestedTypes(field.selections, nestedTypeName, result);
      }
    }
  }
}

function emitCTargetInput(
  target: TargetQuery, schema: SchemaModel, options: CEmitterOptions
): string[] {
  const lines: string[] = [];
  const prefix = target.targetName;

  // Root input type
  lines.push(`typedef struct ${prefix}_Input {`);
  lines.push("    Val __value;");
  lines.push(`} ${prefix}_Input;`);
  lines.push("");

  // Collect all nested wrapper types
  const nestedTypes: { typeName: string; field: QueryFieldSelection; parentPrefix: string }[] = [];
  collectNestedTypes(target.selections, prefix, nestedTypes);

  // Emit nested type declarations
  for (const { typeName, field } of nestedTypes) {
    const hasInlineFragments = (field.inlineFragments?.length ?? 0) > 0;
    if (hasInlineFragments) {
      // Union type: emit tag enum + tagged wrapper
      lines.push(`typedef enum ${typeName}_Tag {`);
      for (const fragment of field.inlineFragments!) {
        lines.push(`    ${typeName}_Tag_${fragment.typeName},`);
      }
      lines.push(`    ${typeName}_Tag_Other,`);
      lines.push(`} ${typeName}_Tag;`);
      lines.push("");
      lines.push(`typedef struct ${typeName} {`);
      lines.push(`    ${typeName}_Tag tag;`);
      lines.push("    Val __value;");
      lines.push(`} ${typeName};`);
      lines.push("");
    } else {
      lines.push(`typedef struct ${typeName} {`);
      lines.push("    Val __value;");
      lines.push(`} ${typeName};`);
      lines.push("");
    }
  }

  // Emit accessor declarations for root fields
  emitCAccessorDecls(target.selections, `${prefix}_Input`, prefix, schema, options, lines);

  // Emit accessor declarations for nested types
  for (const { typeName, field } of nestedTypes) {
    const hasInlineFragments = (field.inlineFragments?.length ?? 0) > 0;
    if (hasInlineFragments) {
      // Union type: emit cast function declarations for each variant
      for (const fragment of field.inlineFragments!) {
        const variantTypeName = `${typeName}_${fragment.typeName}`;
        lines.push(`${variantTypeName} ${typeName}_as_${fragment.typeName}(${typeName} u);`);
      }
    } else {
      emitCAccessorDecls(field.selections, typeName, typeName, schema, options, lines);
      if (isListType(field.schemaType)) {
        lines.push(`uint32_t ${typeName}_len(${typeName.replace(/_Item$/, "")} parent);`);
        lines.push(`${typeName} ${typeName}_get(${typeName.replace(/_Item$/, "")} parent, uint32_t index);`);
      }
    }
  }

  return lines;
}

function emitCAccessorDecls(
  selections: QueryFieldSelection[],
  selfType: string,
  prefix: string,
  schema: SchemaModel,
  options: CEmitterOptions,
  lines: string[]
): void {
  for (const field of selections) {
    const nullable = isNullable(field.schemaType);
    const hasSubSelections = field.selections.length > 0;
    const hasInlineFragments = (field.inlineFragments?.length ?? 0) > 0;
    const isList = isListType(field.schemaType);

    if (hasInlineFragments && !isList) {
      // Union type accessor
      const unionTypeName = `${prefix}_${field.name}`;
      if (nullable) {
        lines.push(`int ${prefix}_has_${field.name}(${selfType} input);`);
      }
      lines.push(`${unionTypeName} ${prefix}_get_${field.name}(${selfType} input);`);
    } else if (isList && hasSubSelections) {
      const itemTypeName = `${prefix}_${field.name}_Item`;
      // For arrays with sub-selections, provide len + get functions
      // The parent accessor returns a wrapper for the array value
      const arrayType = `${prefix}_${field.name}`;
      lines.push(`typedef struct ${arrayType} { Val __value; } ${arrayType};`);
      lines.push(`${arrayType} ${prefix}_get_${field.name}(${selfType} input);`);
      lines.push(`uint32_t ${arrayType}_len(${arrayType} arr);`);
      lines.push(`${itemTypeName} ${arrayType}_get(${arrayType} arr, uint32_t index);`);
    } else if (hasSubSelections) {
      const nestedTypeName = `${prefix}_${field.name}`;
      if (nullable) {
        lines.push(`int ${prefix}_has_${field.name}(${selfType} input);`);
      }
      lines.push(`${nestedTypeName} ${prefix}_get_${field.name}(${selfType} input);`);
    } else {
      const namedType = getNamedType(field.schemaType);
      const cRetType = scalarToCType(namedType, schema, options);
      if (nullable) {
        lines.push(`int ${prefix}_has_${field.name}(${selfType} input);`);
      }
      lines.push(`${cRetType} ${prefix}_get_${field.name}(${selfType} input);`);
    }
  }
}

function emitCTargetInputImpl(
  target: TargetQuery, schema: SchemaModel, options: CEmitterOptions
): string[] {
  const lines: string[] = [];
  const prefix = target.targetName;

  // Root field accessors
  emitCFieldAccessorImpls(target.selections, `${prefix}_Input`, prefix, schema, options, lines);

  // Nested type field accessors
  const nestedTypes: { typeName: string; field: QueryFieldSelection; parentPrefix: string }[] = [];
  collectNestedTypes(target.selections, prefix, nestedTypes);

  for (const { typeName, field } of nestedTypes) {
    emitCFieldAccessorImpls(field.selections, typeName, typeName, schema, options, lines);
  }

  return lines;
}

function emitCFieldAccessorImpls(
  selections: QueryFieldSelection[],
  selfType: string,
  prefix: string,
  schema: SchemaModel,
  options: CEmitterOptions,
  lines: string[]
): void {
  for (const field of selections) {
    const nullable = isNullable(field.schemaType);
    const hasSubSelections = field.selections.length > 0;
    const hasInlineFragments = (field.inlineFragments?.length ?? 0) > 0;
    const isList = isListType(field.schemaType);

    if (hasInlineFragments && !isList) {
      // Union type accessor: reads __typename and returns tagged wrapper
      const unionTypeName = `${prefix}_${field.name}`;

      if (nullable) {
        lines.push(`int ${prefix}_has_${field.name}(${selfType} input) {`);
        lines.push(`    static InternedStringId interned = 0;`);
        lines.push(`    static int initialized = 0;`);
        lines.push(`    if (!initialized) { interned = shopify_function_intern_utf8_str((const uint8_t*)"${field.name}", ${field.name.length}); initialized = 1; }`);
        lines.push(`    Val val = shopify_function_input_get_interned_obj_prop(input.__value, interned);`);
        lines.push(`    return !sf_value_is_null(val);`);
        lines.push("}");
        lines.push("");
      }

      lines.push(`${unionTypeName} ${prefix}_get_${field.name}(${selfType} input) {`);
      lines.push(`    static InternedStringId interned = 0;`);
      lines.push(`    static int initialized = 0;`);
      lines.push(`    if (!initialized) { interned = shopify_function_intern_utf8_str((const uint8_t*)"${field.name}", ${field.name.length}); initialized = 1; }`);
      lines.push(`    Val val = shopify_function_input_get_interned_obj_prop(input.__value, interned);`);
      // Read __typename
      lines.push(`    static InternedStringId tn_interned = 0;`);
      lines.push(`    static int tn_initialized = 0;`);
      lines.push(`    if (!tn_initialized) { tn_interned = shopify_function_intern_utf8_str((const uint8_t*)"__typename", 10); tn_initialized = 1; }`);
      lines.push(`    Val tn_val = shopify_function_input_get_interned_obj_prop(val, tn_interned);`);
      lines.push(`    size_t tn_len = sf_string_len(tn_val);`);
      lines.push(`    uint8_t* tn_buf = sf_bump_alloc(tn_len);`);
      lines.push(`    sf_read_string(tn_val, tn_buf, tn_len);`);
      for (const fragment of field.inlineFragments!) {
        lines.push(`    if (tn_len == ${fragment.typeName.length} && sf_str_eq((const char*)tn_buf, "${fragment.typeName}", ${fragment.typeName.length})) return (${unionTypeName}){ .tag = ${unionTypeName}_Tag_${fragment.typeName}, .__value = val };`);
      }
      lines.push(`    return (${unionTypeName}){ .tag = ${unionTypeName}_Tag_Other, .__value = val };`);
      lines.push("}");
      lines.push("");

      // Cast functions for each variant
      for (const fragment of field.inlineFragments!) {
        const variantTypeName = `${unionTypeName}_${fragment.typeName}`;
        lines.push(`${variantTypeName} ${unionTypeName}_as_${fragment.typeName}(${unionTypeName} u) {`);
        lines.push(`    return (${variantTypeName}){ .__value = u.__value };`);
        lines.push("}");
        lines.push("");
      }
    } else if (isList && hasSubSelections) {
      // Array accessor: returns array wrapper, plus len/get functions
      const arrayType = `${prefix}_${field.name}`;
      const itemTypeName = `${prefix}_${field.name}_Item`;

      // get_<field> returns array wrapper
      lines.push(`${arrayType} ${prefix}_get_${field.name}(${selfType} input) {`);
      lines.push(`    static InternedStringId interned = 0;`);
      lines.push(`    static int initialized = 0;`);
      lines.push(`    if (!initialized) { interned = shopify_function_intern_utf8_str((const uint8_t*)"${field.name}", ${field.name.length}); initialized = 1; }`);
      lines.push(`    Val val = shopify_function_input_get_interned_obj_prop(input.__value, interned);`);
      lines.push(`    return (${arrayType}){ .__value = val };`);
      lines.push("}");
      lines.push("");

      // _len returns array length
      lines.push(`uint32_t ${arrayType}_len(${arrayType} arr) {`);
      lines.push(`    return sf_array_len(arr.__value);`);
      lines.push("}");
      lines.push("");

      // _get returns item at index
      lines.push(`${itemTypeName} ${arrayType}_get(${arrayType} arr, uint32_t index) {`);
      lines.push(`    Val val = shopify_function_input_get_at_index(arr.__value, index);`);
      lines.push(`    return (${itemTypeName}){ .__value = val };`);
      lines.push("}");
      lines.push("");
    } else if (hasSubSelections) {
      // Object accessor: returns nested wrapper
      const nestedTypeName = `${prefix}_${field.name}`;

      if (nullable) {
        lines.push(`int ${prefix}_has_${field.name}(${selfType} input) {`);
        lines.push(`    static InternedStringId interned = 0;`);
        lines.push(`    static int initialized = 0;`);
        lines.push(`    if (!initialized) { interned = shopify_function_intern_utf8_str((const uint8_t*)"${field.name}", ${field.name.length}); initialized = 1; }`);
        lines.push(`    Val val = shopify_function_input_get_interned_obj_prop(input.__value, interned);`);
        lines.push(`    return !sf_value_is_null(val);`);
        lines.push("}");
        lines.push("");
      }

      lines.push(`${nestedTypeName} ${prefix}_get_${field.name}(${selfType} input) {`);
      lines.push(`    static InternedStringId interned = 0;`);
      lines.push(`    static int initialized = 0;`);
      lines.push(`    if (!initialized) { interned = shopify_function_intern_utf8_str((const uint8_t*)"${field.name}", ${field.name.length}); initialized = 1; }`);
      lines.push(`    Val val = shopify_function_input_get_interned_obj_prop(input.__value, interned);`);
      lines.push(`    return (${nestedTypeName}){ .__value = val };`);
      lines.push("}");
      lines.push("");
    } else {
      // Scalar accessor
      const namedType = getNamedType(field.schemaType);
      const cRetType = scalarToCType(namedType, schema, options);

      if (nullable) {
        lines.push(`int ${prefix}_has_${field.name}(${selfType} input) {`);
        lines.push(`    static InternedStringId interned = 0;`);
        lines.push(`    static int initialized = 0;`);
        lines.push(`    if (!initialized) { interned = shopify_function_intern_utf8_str((const uint8_t*)"${field.name}", ${field.name.length}); initialized = 1; }`);
        lines.push(`    Val val = shopify_function_input_get_interned_obj_prop(input.__value, interned);`);
        lines.push(`    return !sf_value_is_null(val);`);
        lines.push("}");
        lines.push("");
      }

      lines.push(`${cRetType} ${prefix}_get_${field.name}(${selfType} input) {`);
      lines.push(`    static InternedStringId interned = 0;`);
      lines.push(`    static int initialized = 0;`);
      lines.push(`    if (!initialized) { interned = shopify_function_intern_utf8_str((const uint8_t*)"${field.name}", ${field.name.length}); initialized = 1; }`);
      lines.push(`    Val val = shopify_function_input_get_interned_obj_prop(input.__value, interned);`);
      lines.push(...emitCScalarAccessorBody(namedType, schema, options));
      lines.push("}");
      lines.push("");
    }
  }
}

function emitCScalarAccessorBody(
  namedType: string, schema: SchemaModel, options: CEmitterOptions
): string[] {
  const lines: string[] = [];
  switch (namedType) {
    case "String": case "ID": case "Date": case "DateTime":
    case "DateTimeWithoutTimezone": case "TimeWithoutTimezone":
    case "Decimal": case "Handle": case "URL": {
      lines.push("    size_t len = sf_string_len(val);");
      lines.push("    uint8_t* buf = sf_bump_alloc(len);");
      lines.push("    sf_read_string(val, buf, len);");
      lines.push("    return (sf_str_t){ .ptr = (const char*)buf, .len = len };");
      break;
    }
    case "Int": {
      lines.push("    return (int32_t)sf_value_as_number(val);");
      break;
    }
    case "Float": {
      lines.push("    return sf_value_as_number(val);");
      break;
    }
    case "Boolean": {
      lines.push("    return sf_value_as_bool(val);");
      break;
    }
    default: {
      if (options.enumsAsStr.includes(namedType)) {
        lines.push("    size_t len = sf_string_len(val);");
        lines.push("    uint8_t* buf = sf_bump_alloc(len);");
        lines.push("    sf_read_string(val, buf, len);");
        lines.push("    return (sf_str_t){ .ptr = (const char*)buf, .len = len };");
      } else if (schema.enumTypes.has(namedType)) {
        lines.push("    size_t len = sf_string_len(val);");
        lines.push("    uint8_t* buf = sf_bump_alloc(len);");
        lines.push("    sf_read_string(val, buf, len);");
        lines.push(`    return ${namedType}_from_str((const char*)buf, len);`);
      } else {
        lines.push("    size_t len = sf_string_len(val);");
        lines.push("    uint8_t* buf = sf_bump_alloc(len);");
        lines.push("    sf_read_string(val, buf, len);");
        lines.push("    return (sf_str_t){ .ptr = (const char*)buf, .len = len };");
      }
    }
  }
  return lines;
}

function isInputType(name: string, schema: SchemaModel): boolean {
  return schema.inputTypes.has(name);
}
