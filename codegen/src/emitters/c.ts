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
import { QueryFieldSelection } from "../parser.js";
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

  // Count non-null fields at runtime
  lines.push("    size_t field_count = 0;");
  for (const field of inputType.fields) {
    const nullable = isNullable(field.type);
    if (nullable && !isListType(field.type)) {
      lines.push(`    if (value->has_${field.name}) field_count++;`);
    } else {
      lines.push("    field_count++;");
    }
  }
  lines.push("    shopify_function_output_new_object(field_count);");

  for (const field of inputType.fields) {
    const nullable = isNullable(field.type) && !isListType(field.type);
    if (nullable) {
      lines.push(`    if (value->has_${field.name}) {`);
    }
    const indent = nullable ? "        " : "    ";
    lines.push(`${indent}shopify_function_output_new_utf8_str((const uint8_t*)"${field.name}", ${field.name.length});`);
    lines.push(...emitCFieldSerialize(field, schema, options, indent));
    if (nullable) {
      lines.push("    }");
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

function emitCTargetInput(
  target: TargetQuery, schema: SchemaModel, options: CEmitterOptions
): string[] {
  const lines: string[] = [];
  const prefix = target.targetName;
  lines.push(`typedef struct ${prefix}_Input {`);
  lines.push("    Val __value;");
  lines.push(`} ${prefix}_Input;`);
  lines.push("");

  for (const field of target.selections) {
    const nullable = isNullable(field.schemaType);
    const namedType = getNamedType(field.schemaType);
    const cRetType = scalarToCType(namedType, schema, options);
    if (nullable) {
      lines.push(`int ${prefix}_input_has_${field.name}(${prefix}_Input input);`);
    }
    lines.push(`${cRetType} ${prefix}_input_get_${field.name}(${prefix}_Input input);`);
  }
  return lines;
}

function emitCTargetInputImpl(
  target: TargetQuery, schema: SchemaModel, options: CEmitterOptions
): string[] {
  const lines: string[] = [];
  const prefix = target.targetName;

  for (const field of target.selections) {
    const nullable = isNullable(field.schemaType);
    const namedType = getNamedType(field.schemaType);
    const cRetType = scalarToCType(namedType, schema, options);

    // has_ function for nullable fields
    if (nullable) {
      lines.push(`int ${prefix}_input_has_${field.name}(${prefix}_Input input) {`);
      lines.push(`    static InternedStringId interned = 0;`);
      lines.push(`    static int initialized = 0;`);
      lines.push(`    if (!initialized) { interned = shopify_function_intern_utf8_str((const uint8_t*)"${field.name}", ${field.name.length}); initialized = 1; }`);
      lines.push(`    Val val = shopify_function_input_get_interned_obj_prop(input.__value, interned);`);
      lines.push(`    return !sf_value_is_null(val);`);
      lines.push("}");
      lines.push("");
    }

    // get_ function
    lines.push(`${cRetType} ${prefix}_input_get_${field.name}(${prefix}_Input input) {`);
    lines.push(`    static InternedStringId interned = 0;`);
    lines.push(`    static int initialized = 0;`);
    lines.push(`    if (!initialized) { interned = shopify_function_intern_utf8_str((const uint8_t*)"${field.name}", ${field.name.length}); initialized = 1; }`);
    lines.push(`    Val val = shopify_function_input_get_interned_obj_prop(input.__value, interned);`);
    lines.push(...emitCScalarAccessorBody(namedType, schema, options));
    lines.push("}");
    lines.push("");
  }
  return lines;
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
