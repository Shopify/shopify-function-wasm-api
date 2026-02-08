/**
 * Go code emitter.
 * Generates Go source code from SchemaModel and parsed queries.
 *
 * Go has interfaces and reflection, so serialization uses a Serialize interface.
 * Generated types implement sf.Serializable.
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

export interface GoEmitterOptions {
  enumsAsStr: string[];
  modulePath: string; // e.g., "github.com/Shopify/shopify-function-go"
  packageName?: string; // defaults to "generated"
}

interface TargetQuery {
  targetName: string;
  graphqlTargetName: string;
  selections: QueryFieldSelection[];
}

export function emitGo(
  schema: SchemaModel,
  targets: TargetQuery[],
  options: GoEmitterOptions
): string {
  const lines: string[] = [];

  lines.push(`package ${options.packageName || "generated"}`);
  lines.push("");
  lines.push("import (");
  lines.push(`\tsf "${options.modulePath}/shopify_function"`);
  lines.push(")");
  lines.push("");

  const inputTypeOrder = getInputTypeOrder(schema);

  // Emit output types
  lines.push("// === Output types ===");
  lines.push("");
  for (const typeName of inputTypeOrder) {
    const inputType = schema.inputTypes.get(typeName)!;
    if (inputType.isOneOf) {
      lines.push(...emitGoOneOf(inputType, schema, options));
    } else {
      lines.push(...emitGoOutputStruct(inputType, schema, options));
    }
    lines.push("");
  }

  // Emit enums
  for (const [name, enumType] of schema.enumTypes) {
    if (options.enumsAsStr.includes(name)) continue;
    lines.push(...emitGoEnum(enumType));
    lines.push("");
  }

  // Emit per-target input types
  lines.push("// === Input types (per-query, lazy accessors) ===");
  lines.push("");
  for (const target of targets) {
    lines.push(...emitGoTargetInput(target, schema, options));
    lines.push("");
  }

  return lines.join("\n");
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

function scalarToGoType(namedType: string, schema: SchemaModel, options: GoEmitterOptions): string {
  switch (namedType) {
    case "String": case "ID": case "Date": case "DateTime":
    case "DateTimeWithoutTimezone": case "TimeWithoutTimezone":
    case "Decimal": case "Handle": case "URL":
      return "string";
    case "Int": return "int32";
    case "Float": return "float64";
    case "Boolean": return "bool";
    default:
      if (options.enumsAsStr.includes(namedType)) return "string";
      if (schema.enumTypes.has(namedType)) return namedType;
      return "string";
  }
}

function typeRefToGo(typeRef: TypeRef, schema: SchemaModel, options: GoEmitterOptions): string {
  switch (typeRef.kind) {
    case "Named": {
      const name = typeRef.name!;
      if (isBuiltinScalar(name) || isScalar(name)) return scalarToGoType(name, schema, options);
      if (options.enumsAsStr.includes(name)) return "string";
      return name;
    }
    case "NonNull": return typeRefToGo(typeRef.ofType!, schema, options);
    case "List": {
      const inner = typeRefToGo(typeRef.ofType!, schema, options);
      return `[]${inner}`;
    }
  }
}

function goFieldName(name: string): string {
  return name.charAt(0).toUpperCase() + name.slice(1);
}

function isListType(typeRef: TypeRef): boolean {
  if (typeRef.kind === "List") return true;
  if (typeRef.kind === "NonNull") return isListType(typeRef.ofType!);
  return false;
}

// --- Output structs ---

function emitGoOutputStruct(
  inputType: InputObjectType, schema: SchemaModel, options: GoEmitterOptions
): string[] {
  const lines: string[] = [];
  lines.push(`type ${inputType.name} struct {`);
  for (const field of inputType.fields) {
    const nullable = isNullable(field.type);
    const goType = typeRefToGo(field.type, schema, options);
    if (nullable) {
      lines.push(`\t${goFieldName(field.name)} *${goType}`);
    } else {
      lines.push(`\t${goFieldName(field.name)} ${goType}`);
    }
  }
  lines.push("}");
  lines.push("");

  // Serialize method
  lines.push(`func (v *${inputType.name}) Serialize() {`);
  // Count non-nil fields
  lines.push("\tfieldCount := uint32(0)");
  for (const field of inputType.fields) {
    const nullable = isNullable(field.type);
    if (nullable) {
      lines.push(`\tif v.${goFieldName(field.name)} != nil { fieldCount++ }`);
    } else {
      lines.push("\tfieldCount++");
    }
  }
  lines.push("\tsf.OutputObject(fieldCount)");
  for (const field of inputType.fields) {
    const nullable = isNullable(field.type);
    if (nullable) {
      lines.push(`\tif v.${goFieldName(field.name)} != nil {`);
      lines.push(`\t\tsf.OutputString("${field.name}")`);
      lines.push(...emitGoFieldSerialize(field, schema, options, true, "\t\t"));
      lines.push("\t}");
    } else {
      lines.push(`\tsf.OutputString("${field.name}")`);
      lines.push(...emitGoFieldSerialize(field, schema, options, false, "\t"));
    }
  }
  lines.push("\tsf.OutputFinishObject()");
  lines.push("}");
  return lines;
}

function emitGoFieldSerialize(
  field: { name: string; type: TypeRef },
  schema: SchemaModel,
  options: GoEmitterOptions,
  deref: boolean,
  indent: string
): string[] {
  const lines: string[] = [];
  const namedType = getNamedType(field.type);
  const isList = isListType(field.type);
  const goField = `v.${goFieldName(field.name)}`;
  const expr = deref ? `*${goField}` : goField;

  if (isList) {
    lines.push(`${indent}sf.OutputArray(uint32(len(${expr})))`);
    lines.push(`${indent}for i := range ${expr} {`);
    if (schema.inputTypes.has(namedType)) {
      lines.push(`${indent}\t${expr}[i].Serialize()`);
    } else {
      lines.push(...emitGoScalarSerialize(`${expr}[i]`, namedType, schema, options, indent + "\t"));
    }
    lines.push(`${indent}}`);
    lines.push(`${indent}sf.OutputFinishArray()`);
  } else if (schema.inputTypes.has(namedType)) {
    lines.push(`${indent}${expr}.Serialize()`);
  } else {
    lines.push(...emitGoScalarSerialize(expr, namedType, schema, options, indent));
  }
  return lines;
}

function emitGoScalarSerialize(
  expr: string, namedType: string, schema: SchemaModel, options: GoEmitterOptions, indent: string
): string[] {
  const lines: string[] = [];
  switch (namedType) {
    case "String": case "ID": case "Date": case "DateTime":
    case "DateTimeWithoutTimezone": case "TimeWithoutTimezone":
    case "Decimal": case "Handle": case "URL":
      lines.push(`${indent}sf.OutputString(${expr})`);
      break;
    case "Int":
      lines.push(`${indent}sf.OutputI32(${expr})`);
      break;
    case "Float":
      lines.push(`${indent}sf.OutputF64(${expr})`);
      break;
    case "Boolean":
      lines.push(`${indent}sf.OutputBool(${expr})`);
      break;
    default:
      if (options.enumsAsStr.includes(namedType)) {
        lines.push(`${indent}sf.OutputString(${expr})`);
      } else if (schema.enumTypes.has(namedType)) {
        lines.push(`${indent}sf.OutputString(${expr}.String())`);
      } else {
        lines.push(`${indent}sf.OutputString(${expr})`);
      }
  }
  return lines;
}

// --- @oneOf ---

function emitGoOneOf(
  inputType: InputObjectType, schema: SchemaModel, options: GoEmitterOptions
): string[] {
  const lines: string[] = [];
  // Interface
  lines.push(`type ${inputType.name} interface {`);
  lines.push(`\tis${inputType.name}()`);
  lines.push(`\tSerialize()`);
  lines.push("}");
  lines.push("");

  // Variant types
  for (const field of inputType.fields) {
    const namedType = getNamedType(field.type);
    const variantName = `${inputType.name}_${goFieldName(field.name)}`;
    const goType = schema.inputTypes.has(namedType)
      ? namedType
      : scalarToGoType(namedType, schema, options);

    lines.push(`type ${variantName} struct {`);
    lines.push(`\tValue ${goType}`);
    lines.push("}");
    lines.push(`func (${variantName}) is${inputType.name}() {}`);
    lines.push(`func (v ${variantName}) Serialize() {`);
    lines.push("\tsf.OutputObject(1)");
    lines.push(`\tsf.OutputString("${field.name}")`);
    if (schema.inputTypes.has(namedType)) {
      lines.push("\tv.Value.Serialize()");
    } else {
      lines.push(...emitGoScalarSerialize("v.Value", namedType, schema, options, "\t"));
    }
    lines.push("\tsf.OutputFinishObject()");
    lines.push("}");
    lines.push("");
  }
  return lines;
}

// --- Enums ---

function emitGoEnum(enumType: EnumType): string[] {
  const lines: string[] = [];
  lines.push(`type ${enumType.name} int`);
  lines.push("");
  lines.push("const (");
  for (let i = 0; i < enumType.values.length; i++) {
    const value = enumType.values[i];
    if (i === 0) {
      lines.push(`\t${enumType.name}${value} ${enumType.name} = iota`);
    } else {
      lines.push(`\t${enumType.name}${value}`);
    }
  }
  lines.push(`\t${enumType.name}Other`);
  lines.push(")");
  lines.push("");

  // String method
  lines.push(`func (e ${enumType.name}) String() string {`);
  lines.push("\tswitch e {");
  for (const value of enumType.values) {
    lines.push(`\tcase ${enumType.name}${value}: return "${value}"`);
  }
  lines.push(`\tdefault: return ""`);
  lines.push("\t}");
  lines.push("}");
  lines.push("");

  // FromStr
  lines.push(`func ${enumType.name}FromStr(s string) ${enumType.name} {`);
  lines.push("\tswitch s {");
  for (const value of enumType.values) {
    lines.push(`\tcase "${value}": return ${enumType.name}${value}`);
  }
  lines.push(`\tdefault: return ${enumType.name}Other`);
  lines.push("\t}");
  lines.push("}");
  return lines;
}

// --- Input accessors ---

function emitGoTargetInput(
  target: TargetQuery, schema: SchemaModel, options: GoEmitterOptions
): string[] {
  const lines: string[] = [];
  const typeName = goFieldName(target.targetName) + "Input";

  lines.push(`type ${typeName} struct {`);
  lines.push("\tValue sf.Value");
  lines.push("}");
  lines.push("");

  for (const field of target.selections) {
    const nullable = isNullable(field.schemaType);
    const namedType = getNamedType(field.schemaType);
    const goRetType = scalarToGoType(namedType, schema, options);
    const methodName = goFieldName(field.name);

    if (nullable) {
      lines.push(`func (input ${typeName}) ${methodName}() (*${goRetType}, bool) {`);
    } else {
      lines.push(`func (input ${typeName}) ${methodName}() ${goRetType} {`);
    }

    // Interned string lookup - use package-level var
    lines.push(`\tval := input.Value.GetObjProp("${field.name}")`);

    if (nullable) {
      lines.push("\tif val.IsNull() { return nil, false }");
    }

    lines.push(...emitGoScalarAccessorBody(namedType, nullable, schema, options));
    lines.push("}");
    lines.push("");
  }

  return lines;
}

function emitGoScalarAccessorBody(
  namedType: string, nullable: boolean, schema: SchemaModel, options: GoEmitterOptions
): string[] {
  const lines: string[] = [];
  switch (namedType) {
    case "String": case "ID": case "Date": case "DateTime":
    case "DateTimeWithoutTimezone": case "TimeWithoutTimezone":
    case "Decimal": case "Handle": case "URL": {
      lines.push("\ts := val.ReadStringAlloc()");
      if (nullable) {
        lines.push("\treturn &s, true");
      } else {
        lines.push("\treturn s");
      }
      break;
    }
    case "Int": {
      lines.push("\tn, _ := val.AsNumber()");
      lines.push("\tresult := int32(n)");
      if (nullable) {
        lines.push("\treturn &result, true");
      } else {
        lines.push("\treturn result");
      }
      break;
    }
    case "Float": {
      lines.push("\tn, _ := val.AsNumber()");
      if (nullable) {
        lines.push("\treturn &n, true");
      } else {
        lines.push("\treturn n");
      }
      break;
    }
    case "Boolean": {
      lines.push("\tb, _ := val.AsBool()");
      if (nullable) {
        lines.push("\treturn &b, true");
      } else {
        lines.push("\treturn b");
      }
      break;
    }
    default: {
      if (options.enumsAsStr.includes(namedType)) {
        lines.push("\ts := val.ReadStringAlloc()");
        if (nullable) {
          lines.push("\treturn &s, true");
        } else {
          lines.push("\treturn s");
        }
      } else if (schema.enumTypes.has(namedType)) {
        lines.push("\ts := val.ReadStringAlloc()");
        lines.push(`\tresult := ${namedType}FromStr(s)`);
        if (nullable) {
          lines.push("\treturn &result, true");
        } else {
          lines.push("\treturn result");
        }
      } else {
        lines.push("\ts := val.ReadStringAlloc()");
        if (nullable) {
          lines.push("\treturn &s, true");
        } else {
          lines.push("\treturn s");
        }
      }
    }
  }
  return lines;
}
