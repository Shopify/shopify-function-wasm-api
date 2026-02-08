/**
 * Zig code emitter.
 * Generates Zig source code from SchemaModel and parsed queries.
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

export interface ZigEmitterOptions {
  enumsAsStr: string[]; // Enum names to treat as plain strings
}

interface TargetQuery {
  targetName: string; // e.g., "target_a" (snake_case)
  graphqlTargetName: string; // e.g., "targetA" (camelCase)
  selections: QueryFieldSelection[];
}

/**
 * Generate a complete schema.zig file from a schema model and queries.
 */
export function emitZig(
  schema: SchemaModel,
  targets: TargetQuery[],
  options: ZigEmitterOptions
): string {
  const lines: string[] = [];

  lines.push("const sf = @import(\"shopify_function\");");
  lines.push("");
  lines.push("// === Output types (from GraphQL input types) ===");
  lines.push("");

  // Emit output types (GraphQL input types)
  // Sort to ensure deterministic output: result types first, then dependencies
  const emittedTypes = new Set<string>();
  const inputTypeOrder = getInputTypeOrder(schema);

  for (const typeName of inputTypeOrder) {
    const inputType = schema.inputTypes.get(typeName)!;
    if (inputType.isOneOf) {
      lines.push(...emitOneOfUnion(inputType, schema, options));
    } else {
      lines.push(...emitOutputStruct(inputType, schema, options));
    }
    emittedTypes.add(typeName);
    lines.push("");
  }

  // Emit enum types
  const hasEnums = schema.enumTypes.size > 0;
  if (hasEnums) {
    lines.push("// === Enums ===");
    lines.push("");
    for (const [name, enumType] of schema.enumTypes) {
      if (options.enumsAsStr.includes(name)) continue;
      lines.push(...emitEnum(enumType));
      lines.push("");
    }
  }

  // Emit per-target input types
  lines.push("// === Input types (per-query, lazy accessors) ===");
  lines.push("");

  for (const target of targets) {
    lines.push(...emitTargetModule(target, schema, options));
    lines.push("");
  }

  return lines.join("\n");
}

/** Get input types in dependency order */
function getInputTypeOrder(schema: SchemaModel): string[] {
  const visited = new Set<string>();
  const order: string[] = [];

  function visit(typeName: string) {
    if (visited.has(typeName)) return;
    visited.add(typeName);

    const inputType = schema.inputTypes.get(typeName);
    if (!inputType) return;

    // Visit dependencies first
    for (const field of inputType.fields) {
      const depName = getNamedType(field.type);
      if (schema.inputTypes.has(depName)) {
        visit(depName);
      }
    }

    order.push(typeName);
  }

  for (const typeName of schema.inputTypes.keys()) {
    visit(typeName);
  }

  return order;
}

/** Emit a struct for a non-@oneOf input type */
function emitOutputStruct(
  inputType: InputObjectType,
  schema: SchemaModel,
  options: ZigEmitterOptions
): string[] {
  const lines: string[] = [];
  lines.push(`pub const ${inputType.name} = struct {`);

  for (const field of inputType.fields) {
    const baseZigType = typeRefToZig(field.type, schema, options);
    const nullable = isNullable(field.type);
    const zigType = nullable ? `?${baseZigType}` : baseZigType;
    const defaultValue = getDefaultValue(field.type, schema, options);
    if (defaultValue !== null) {
      lines.push(`    ${field.name}: ${zigType} = ${defaultValue},`);
    } else {
      lines.push(`    ${field.name}: ${zigType},`);
    }
  }

  lines.push("};");
  return lines;
}

/** Emit a tagged union for a @oneOf input type */
function emitOneOfUnion(
  inputType: InputObjectType,
  schema: SchemaModel,
  options: ZigEmitterOptions
): string[] {
  const lines: string[] = [];
  lines.push(`pub const ${inputType.name} = union(enum) {`);

  for (const field of inputType.fields) {
    // For @oneOf, each field's type is the payload type (unwrap NonNull)
    const payloadType = typeRefToZig(field.type, schema, options, true);
    lines.push(`    ${field.name}: ${payloadType},`);
  }

  lines.push("};");
  return lines;
}

/** Emit an enum type with fromStr/asStr */
function emitEnum(enumType: EnumType): string[] {
  const lines: string[] = [];
  lines.push(`pub const ${enumType.name} = enum {`);

  for (const value of enumType.values) {
    lines.push(`    ${value},`);
  }
  // Add Other variant for forward compatibility
  lines.push("    Other,");
  lines.push("");

  // fromStr
  lines.push("    pub fn fromStr(s: []const u8) @This() {");
  lines.push("        const map = .{");
  for (const value of enumType.values) {
    lines.push(`            .{ "${value}", .${value} },`);
  }
  lines.push("        };");
  lines.push("        inline for (map) |entry| {");
  lines.push("            if (sf.wasm.strEql(s, entry[0])) return entry[1];");
  lines.push("        }");
  lines.push("        return .Other;");
  lines.push("    }");
  lines.push("");

  // asStr
  lines.push('    pub fn asStr(self: @This()) []const u8 {');
  lines.push("        return switch (self) {");
  for (const value of enumType.values) {
    lines.push(`            .${value} => "${value}",`);
  }
  lines.push('            .Other => unreachable,');
  lines.push("        };");
  lines.push("    }");

  lines.push("};");
  return lines;
}

/** Emit a target module with its Input type */
function emitTargetModule(
  target: TargetQuery,
  schema: SchemaModel,
  options: ZigEmitterOptions
): string[] {
  const lines: string[] = [];

  lines.push(`pub const ${target.targetName} = struct {`);
  lines.push("    pub const Input = struct {");
  lines.push("        __value: sf.wasm.Value,");
  lines.push("");

  for (const field of target.selections) {
    lines.push(
      ...emitAccessor(field, schema, options, "        ")
    );
    lines.push("");
  }

  lines.push("    };");
  lines.push("};");

  return lines;
}

/** Emit a single field accessor */
function emitAccessor(
  field: QueryFieldSelection,
  schema: SchemaModel,
  options: ZigEmitterOptions,
  indent: string
): string[] {
  const lines: string[] = [];
  const nullable = isNullable(field.schemaType);
  const namedType = getNamedType(field.schemaType);

  // Check if this field has sub-selections (is an object/complex type)
  const hasSubSelections = field.selections.length > 0;

  // Determine if this is a list type
  const isList = isListType(field.schemaType);

  // Determine return type
  let returnType: string;
  if (isList) {
    const innerTypeRef = getListInnerType(field.schemaType);
    if (hasSubSelections) {
      returnType = `sf.ArrayAccessor(${field.name}_Item)`;
    } else {
      const innerZig = typeRefToZigForAccessor(innerTypeRef, schema, options);
      returnType = `sf.ArrayAccessor(${innerZig})`;
    }
    if (nullable) returnType = `?${returnType}`;
  } else if (hasSubSelections) {
    returnType = `${field.name}_T`;
    if (nullable) returnType = `?${returnType}`;
  } else {
    returnType = scalarAccessorReturnType(namedType, nullable, schema, options);
  }

  // Generate the accessor function
  lines.push(`${indent}pub fn ${field.name}(self: Input) ${returnType} {`);

  // Interned string lookup
  lines.push(
    `${indent}    const S = struct { var interned: ?sf.wasm.InternedStringId = null; };`
  );
  lines.push(
    `${indent}    if (S.interned == null) S.interned = sf.wasm.internString("${field.name}");`
  );
  lines.push(
    `${indent}    const val = self.__value.getInternedObjProp(S.interned.?);`
  );

  if (isList) {
    if (nullable) {
      lines.push(`${indent}    if (val.isNull()) return null;`);
      if (hasSubSelections) {
        lines.push(
          `${indent}    return sf.ArrayAccessor(${field.name}_Item).init(val);`
        );
      } else {
        const innerTypeRef = getListInnerType(field.schemaType);
        const innerZig = typeRefToZigForAccessor(innerTypeRef, schema, options);
        lines.push(
          `${indent}    return sf.ArrayAccessor(${innerZig}).init(val);`
        );
      }
    } else {
      if (hasSubSelections) {
        lines.push(
          `${indent}    return sf.ArrayAccessor(${field.name}_Item).init(val);`
        );
      } else {
        const innerTypeRef = getListInnerType(field.schemaType);
        const innerZig = typeRefToZigForAccessor(innerTypeRef, schema, options);
        lines.push(
          `${indent}    return sf.ArrayAccessor(${innerZig}).init(val);`
        );
      }
    }
  } else if (hasSubSelections) {
    if (nullable) {
      lines.push(`${indent}    if (val.isNull()) return null;`);
    }
    lines.push(`${indent}    return .{ .__value = val };`);
  } else {
    // Scalar field accessor body
    lines.push(
      ...emitScalarAccessorBody(namedType, nullable, schema, options, indent)
    );
  }

  lines.push(`${indent}}`);

  // If this field has sub-selections, emit a nested type
  if (hasSubSelections && !isList) {
    lines.push("");
    lines.push(`${indent}pub const ${field.name}_T = struct {`);
    lines.push(`${indent}    __value: sf.wasm.Value,`);
    lines.push("");
    for (const subField of field.selections) {
      const nestedInput = `${indent}    `;
      lines.push(`${nestedInput}pub fn ${subField.name}(self: ${field.name}_T) ${getAccessorReturnType(subField, schema, options)} {`);
      lines.push(`${nestedInput}    const S = struct { var interned: ?sf.wasm.InternedStringId = null; };`);
      lines.push(`${nestedInput}    if (S.interned == null) S.interned = sf.wasm.internString("${subField.name}");`);
      lines.push(`${nestedInput}    const val = self.__value.getInternedObjProp(S.interned.?);`);
      const subNullable = isNullable(subField.schemaType);
      const subNamedType = getNamedType(subField.schemaType);
      lines.push(...emitScalarAccessorBody(subNamedType, subNullable, schema, options, nestedInput));
      lines.push(`${nestedInput}}`);
      lines.push("");
    }
    lines.push(`${indent}};`);
  }

  // If this is a list with sub-selections, emit the Item type
  if (hasSubSelections && isList) {
    lines.push("");
    lines.push(`${indent}pub const ${field.name}_Item = struct {`);
    lines.push(`${indent}    __value: sf.wasm.Value,`);
    lines.push("");
    // ArrayAccessor uses fromValue to construct items
    lines.push(`${indent}    pub fn fromValue(val: sf.wasm.Value) ${field.name}_Item {`);
    lines.push(`${indent}        return .{ .__value = val };`);
    lines.push(`${indent}    }`);
    lines.push("");
    for (const subField of field.selections) {
      const nestedInput = `${indent}    `;
      lines.push(`${nestedInput}pub fn ${subField.name}(self: ${field.name}_Item) ${getAccessorReturnType(subField, schema, options)} {`);
      lines.push(`${nestedInput}    const S = struct { var interned: ?sf.wasm.InternedStringId = null; };`);
      lines.push(`${nestedInput}    if (S.interned == null) S.interned = sf.wasm.internString("${subField.name}");`);
      lines.push(`${nestedInput}    const val = self.__value.getInternedObjProp(S.interned.?);`);
      const subNullable = isNullable(subField.schemaType);
      const subNamedType = getNamedType(subField.schemaType);
      lines.push(...emitScalarAccessorBody(subNamedType, subNullable, schema, options, nestedInput));
      lines.push(`${nestedInput}}`);
      lines.push("");
    }
    lines.push(`${indent}};`);
  }

  return lines;
}

function getAccessorReturnType(
  field: QueryFieldSelection,
  schema: SchemaModel,
  options: ZigEmitterOptions
): string {
  const nullable = isNullable(field.schemaType);
  const namedType = getNamedType(field.schemaType);
  return scalarAccessorReturnType(namedType, nullable, schema, options);
}

/** Determine return type for a scalar accessor */
function scalarAccessorReturnType(
  namedType: string,
  nullable: boolean,
  schema: SchemaModel,
  options: ZigEmitterOptions
): string {
  const baseType = scalarToZigType(namedType, schema, options);
  if (nullable) return `?${baseType}`;
  return baseType;
}

/** Emit the body of a scalar accessor (after val is obtained) */
function emitScalarAccessorBody(
  namedType: string,
  nullable: boolean,
  schema: SchemaModel,
  options: ZigEmitterOptions,
  indent: string
): string[] {
  const lines: string[] = [];

  if (nullable) {
    lines.push(`${indent}    if (val.isNull()) return null;`);
  }

  switch (namedType) {
    case "String":
    case "ID":
    case "Date":
    case "DateTime":
    case "DateTimeWithoutTimezone":
    case "TimeWithoutTimezone":
    case "Decimal":
    case "Handle":
    case "URL": {
      lines.push(`${indent}    const len = val.stringLen();`);
      lines.push(`${indent}    const str_buf = sf.buf(len);`);
      lines.push(`${indent}    val.readString(str_buf);`);
      lines.push(`${indent}    return str_buf;`);
      break;
    }
    case "Int": {
      lines.push(`${indent}    const n = val.asNumber() orelse ${nullable ? "return null" : "unreachable"};`);
      lines.push(`${indent}    return @intFromFloat(n);`);
      break;
    }
    case "Float": {
      lines.push(`${indent}    return val.asNumber()${nullable ? "" : " orelse unreachable"};`);
      break;
    }
    case "Boolean": {
      lines.push(`${indent}    return val.asBool()${nullable ? "" : " orelse unreachable"};`);
      break;
    }
    default: {
      // Check if it's an enum treated as string
      if (options.enumsAsStr.includes(namedType)) {
        lines.push(`${indent}    const len = val.stringLen();`);
        lines.push(`${indent}    const str_buf = sf.buf(len);`);
        lines.push(`${indent}    val.readString(str_buf);`);
        lines.push(`${indent}    return str_buf;`);
      } else if (schema.enumTypes.has(namedType)) {
        // Enum with real variants
        lines.push(`${indent}    const len = val.stringLen();`);
        lines.push(`${indent}    const str_buf = sf.buf(len);`);
        lines.push(`${indent}    val.readString(str_buf);`);
        lines.push(`${indent}    return ${namedType}.fromStr(str_buf);`);
      } else {
        // Unknown type - treat as string
        lines.push(`${indent}    const len = val.stringLen();`);
        lines.push(`${indent}    const str_buf = sf.buf(len);`);
        lines.push(`${indent}    val.readString(str_buf);`);
        lines.push(`${indent}    return str_buf;`);
      }
    }
  }

  return lines;
}

/** Map a scalar GraphQL type to Zig type */
function scalarToZigType(
  namedType: string,
  schema: SchemaModel,
  options: ZigEmitterOptions
): string {
  switch (namedType) {
    case "String":
    case "ID":
    case "Date":
    case "DateTime":
    case "DateTimeWithoutTimezone":
    case "TimeWithoutTimezone":
    case "Handle":
    case "URL":
      return "[]const u8";
    case "Decimal":
      return "[]const u8"; // Decimal is string-backed
    case "Int":
      return "i32";
    case "Float":
      return "f64";
    case "Boolean":
      return "bool";
    case "Json":
    case "JSON":
      return "sf.scalars.JsonValue";
    case "Void":
      return "void";
    default:
      if (options.enumsAsStr.includes(namedType)) {
        return "[]const u8";
      }
      if (schema.enumTypes.has(namedType)) {
        return namedType;
      }
      return "[]const u8"; // fallback
  }
}

/** Convert a TypeRef to a Zig type string (for output/serialization types) */
function typeRefToZig(
  typeRef: TypeRef,
  schema: SchemaModel,
  options: ZigEmitterOptions,
  unwrapOneOfNonNull: boolean = false
): string {
  switch (typeRef.kind) {
    case "Named": {
      const name = typeRef.name!;
      // Check if it maps to a scalar
      if (isBuiltinScalar(name) || isScalar(name)) {
        return scalarToZigType(name, schema, options);
      }
      // Check if it's an enum treated as string
      if (options.enumsAsStr.includes(name)) {
        return "[]const u8";
      }
      // Check if it's an enum
      if (schema.enumTypes.has(name)) {
        return name;
      }
      // Otherwise it's a type name (struct or union)
      return name;
    }
    case "NonNull": {
      if (unwrapOneOfNonNull) {
        return typeRefToZig(typeRef.ofType!, schema, options);
      }
      return typeRefToZig(typeRef.ofType!, schema, options);
    }
    case "List": {
      const inner = typeRefToZig(typeRef.ofType!, schema, options);
      return `[]const ${inner}`;
    }
  }
}

/** Get default value for an output struct field */
function getDefaultValue(
  typeRef: TypeRef,
  schema: SchemaModel,
  options: ZigEmitterOptions
): string | null {
  if (isNullable(typeRef)) {
    return "null";
  }
  return null; // Non-null fields have no default
}

/** Check if a TypeRef is a list type (unwrapping NonNull) */
function isListType(typeRef: TypeRef): boolean {
  if (typeRef.kind === "List") return true;
  if (typeRef.kind === "NonNull") return isListType(typeRef.ofType!);
  return false;
}

/** Get the inner type of a list (unwrapping NonNull and List wrappers) */
function getListInnerType(typeRef: TypeRef): TypeRef {
  if (typeRef.kind === "NonNull") return getListInnerType(typeRef.ofType!);
  if (typeRef.kind === "List") return typeRef.ofType!;
  return typeRef;
}

/** Map a TypeRef to Zig type for use in ArrayAccessor */
function typeRefToZigForAccessor(
  typeRef: TypeRef,
  schema: SchemaModel,
  options: ZigEmitterOptions
): string {
  const namedType = getNamedType(typeRef);
  return scalarToZigType(namedType, schema, options);
}

/** Helper to convert camelCase to snake_case */
export function camelToSnake(name: string): string {
  return name
    .replace(/([A-Z])/g, "_$1")
    .toLowerCase()
    .replace(/^_/, "");
}
