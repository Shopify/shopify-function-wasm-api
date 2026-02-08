/**
 * Internal type model extracted from a GraphQL schema.
 * Represents the types needed for code generation.
 */

export interface FieldDefinition {
  name: string;
  type: TypeRef;
  description?: string;
  restrictTargets?: string[]; // from @restrictTarget(only: [...])
}

export interface TypeRef {
  kind: "Named" | "NonNull" | "List";
  name?: string; // for Named
  ofType?: TypeRef; // for NonNull and List
}

export interface ObjectType {
  name: string;
  fields: FieldDefinition[];
}

export interface InputObjectType {
  name: string;
  fields: FieldDefinition[];
  isOneOf: boolean; // @oneOf directive
}

export interface EnumType {
  name: string;
  values: string[];
}

export interface UnionType {
  name: string;
  memberTypes: string[]; // e.g., ["CustomProduct", "ProductVariant"]
}

export interface MutationTarget {
  name: string; // e.g., "targetA"
  resultTypeName: string; // e.g., "FunctionTargetAResult"
}

export interface SchemaModel {
  queryType: ObjectType | null;
  mutationTargets: MutationTarget[];
  objectTypes: Map<string, ObjectType>;
  inputTypes: Map<string, InputObjectType>;
  enumTypes: Map<string, EnumType>;
  unionTypes: Map<string, UnionType>;
  customScalars: Set<string>;
}

/** Helper to create a non-null type ref */
export function nonNull(inner: TypeRef): TypeRef {
  return { kind: "NonNull", ofType: inner };
}

/** Helper to create a list type ref */
export function listOf(inner: TypeRef): TypeRef {
  return { kind: "List", ofType: inner };
}

/** Helper to create a named type ref */
export function named(name: string): TypeRef {
  return { kind: "Named", name };
}

/** Unwrap NonNull wrappers to get the base type ref */
export function unwrapNonNull(typeRef: TypeRef): TypeRef {
  if (typeRef.kind === "NonNull") {
    return typeRef.ofType!;
  }
  return typeRef;
}

/** Check if a type ref is nullable (not wrapped in NonNull) */
export function isNullable(typeRef: TypeRef): boolean {
  return typeRef.kind !== "NonNull";
}

/** Get the leaf named type from a type ref */
export function getNamedType(typeRef: TypeRef): string {
  switch (typeRef.kind) {
    case "Named":
      return typeRef.name!;
    case "NonNull":
    case "List":
      return getNamedType(typeRef.ofType!);
  }
}

/** Check if a type is a built-in scalar */
export function isBuiltinScalar(name: string): boolean {
  return ["String", "Int", "Float", "Boolean", "ID"].includes(name);
}

/** Check if a type is a known custom scalar */
export function isCustomScalar(name: string): boolean {
  return [
    "Date",
    "DateTime",
    "DateTimeWithoutTimezone",
    "TimeWithoutTimezone",
    "Decimal",
    "Void",
    "Json",
    "JSON",
    "URL",
    "Handle",
  ].includes(name);
}

/** Check if a type is any scalar (built-in or custom) */
export function isScalar(name: string): boolean {
  return isBuiltinScalar(name) || isCustomScalar(name);
}
