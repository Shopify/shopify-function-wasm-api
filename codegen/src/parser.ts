/**
 * GraphQL schema and query parser using the `graphql` npm package.
 * Produces an internal SchemaModel for code generation.
 */

import {
  buildSchema,
  GraphQLSchema,
  GraphQLObjectType,
  GraphQLInputObjectType,
  GraphQLEnumType,
  GraphQLField,
  GraphQLInputField,
  GraphQLType,
  GraphQLNonNull,
  GraphQLList,
  GraphQLNamedType,
  GraphQLScalarType,
  GraphQLUnionType,
  isObjectType,
  isInputObjectType,
  isEnumType,
  isScalarType,
  isUnionType,
  isNonNullType,
  isListType,
  parse,
  DocumentNode,
  SelectionSetNode,
  FieldNode,
  InlineFragmentNode,
} from "graphql";

import {
  SchemaModel,
  ObjectType,
  InputObjectType,
  EnumType,
  UnionType,
  MutationTarget,
  FieldDefinition,
  TypeRef,
  named,
  nonNull,
  listOf,
} from "./schema-model.js";

/**
 * Parse a GraphQL schema string into a SchemaModel.
 */
export function parseSchema(schemaSource: string): SchemaModel {
  const schema = buildSchema(schemaSource);

  const model: SchemaModel = {
    queryType: null,
    mutationTargets: [],
    objectTypes: new Map(),
    inputTypes: new Map(),
    enumTypes: new Map(),
    unionTypes: new Map(),
    customScalars: new Set(),
  };

  // Extract query type (the Input type for functions)
  const queryType = schema.getQueryType();
  if (queryType) {
    model.queryType = extractObjectType(queryType, schema);
    model.objectTypes.set(queryType.name, model.queryType);
  }

  // Extract mutation targets
  const mutationType = schema.getMutationType();
  if (mutationType) {
    const fields = mutationType.getFields();
    for (const [fieldName, field] of Object.entries(fields)) {
      const args = field.args;
      if (args.length > 0) {
        const resultArg = args.find((a) => a.name === "result");
        if (resultArg) {
          const resultTypeName = getNamedTypeName(resultArg.type);
          model.mutationTargets.push({
            name: fieldName,
            resultTypeName,
          });
        }
      }
    }
  }

  // Extract all types from the schema
  const typeMap = schema.getTypeMap();
  for (const [typeName, type] of Object.entries(typeMap)) {
    // Skip built-in types
    if (typeName.startsWith("__")) continue;
    if (
      ["String", "Int", "Float", "Boolean", "ID"].includes(typeName)
    )
      continue;

    if (isObjectType(type) && type !== queryType && type !== mutationType) {
      model.objectTypes.set(typeName, extractObjectType(type, schema));
    } else if (isInputObjectType(type)) {
      model.inputTypes.set(typeName, extractInputObjectType(type));
    } else if (isEnumType(type)) {
      model.enumTypes.set(typeName, extractEnumType(type));
    } else if (isUnionType(type)) {
      model.unionTypes.set(typeName, extractUnionType(type));
    } else if (isScalarType(type)) {
      if (!["String", "Int", "Float", "Boolean", "ID"].includes(typeName)) {
        model.customScalars.add(typeName);
      }
    }
  }

  return model;
}

function extractObjectType(
  type: GraphQLObjectType,
  schema: GraphQLSchema
): ObjectType {
  const fields: FieldDefinition[] = [];
  const fieldMap = type.getFields();

  for (const [fieldName, field] of Object.entries(fieldMap)) {
    const fieldDef: FieldDefinition = {
      name: fieldName,
      type: graphqlTypeToTypeRef(field.type),
    };

    // Extract @restrictTarget directive
    const restrictTarget = field.astNode?.directives?.find(
      (d) => d.name.value === "restrictTarget"
    );
    if (restrictTarget) {
      const onlyArg = restrictTarget.arguments?.find(
        (a) => a.name.value === "only"
      );
      if (onlyArg && onlyArg.value.kind === "ListValue") {
        fieldDef.restrictTargets = onlyArg.value.values
          .filter((v) => v.kind === "StringValue")
          .map((v) => (v as any).value as string);
      }
    }

    if (field.description) {
      fieldDef.description = field.description;
    }

    fields.push(fieldDef);
  }

  return { name: type.name, fields };
}

function extractInputObjectType(type: GraphQLInputObjectType): InputObjectType {
  const fields: FieldDefinition[] = [];
  const fieldMap = type.getFields();

  for (const [fieldName, field] of Object.entries(fieldMap)) {
    const fieldDef: FieldDefinition = {
      name: fieldName,
      type: graphqlTypeToTypeRef(field.type),
    };

    if (field.description) {
      fieldDef.description = field.description;
    }

    fields.push(fieldDef);
  }

  // Check for @oneOf directive
  const isOneOf = type.astNode?.directives?.some(
    (d) => d.name.value === "oneOf"
  ) ?? false;

  return { name: type.name, fields, isOneOf };
}

function extractEnumType(type: GraphQLEnumType): EnumType {
  return {
    name: type.name,
    values: type.getValues().map((v) => v.name),
  };
}

function extractUnionType(type: GraphQLUnionType): UnionType {
  return {
    name: type.name,
    memberTypes: type.getTypes().map((t) => t.name),
  };
}

function graphqlTypeToTypeRef(type: GraphQLType): TypeRef {
  if (isNonNullType(type)) {
    return nonNull(graphqlTypeToTypeRef(type.ofType));
  }
  if (isListType(type)) {
    return listOf(graphqlTypeToTypeRef(type.ofType));
  }
  return named((type as GraphQLNamedType).name);
}

function getNamedTypeName(type: GraphQLType): string {
  if (isNonNullType(type)) {
    return getNamedTypeName(type.ofType);
  }
  if (isListType(type)) {
    return getNamedTypeName(type.ofType);
  }
  return (type as GraphQLNamedType).name;
}

/**
 * Represents an inline fragment selection within a union field.
 */
export interface InlineFragmentSelection {
  typeName: string; // e.g., "ProductVariant"
  selections: QueryFieldSelection[];
}

/**
 * Represents a field selection from a query, with its resolved schema type.
 */
export interface QueryFieldSelection {
  name: string; // field name as it appears in the query (alias if present)
  schemaType: TypeRef; // type from the schema
  selections: QueryFieldSelection[]; // sub-selections for object fields
  inlineFragments?: InlineFragmentSelection[]; // for union type fields
}

/**
 * Represents a parsed query with its selection tree.
 */
export interface ParsedQuery {
  selections: QueryFieldSelection[];
}

/**
 * Parse a GraphQL query string and resolve types against the schema model.
 */
export function parseQuery(
  querySource: string,
  schemaModel: SchemaModel,
  targetName?: string
): ParsedQuery {
  const doc: DocumentNode = parse(querySource);
  const operationDef = doc.definitions.find(
    (d) => d.kind === "OperationDefinition"
  );

  if (!operationDef || operationDef.kind !== "OperationDefinition") {
    throw new Error("No operation definition found in query");
  }

  const queryType = schemaModel.queryType;
  if (!queryType) {
    throw new Error("No query type found in schema");
  }

  const selections = resolveSelections(
    operationDef.selectionSet,
    queryType,
    schemaModel,
    targetName
  );

  return { selections };
}

function resolveSelections(
  selectionSet: SelectionSetNode,
  parentType: ObjectType,
  schemaModel: SchemaModel,
  targetName?: string
): QueryFieldSelection[] {
  const result: QueryFieldSelection[] = [];

  for (const selection of selectionSet.selections) {
    if (selection.kind !== "Field") continue;

    const fieldNode = selection as FieldNode;
    const schemaFieldName = fieldNode.name.value;

    // Skip __typename — it's handled implicitly for union dispatch
    if (schemaFieldName === "__typename") continue;

    // Use alias as the accessor name if present (alias matches the key in input data)
    const accessorName = fieldNode.alias?.value ?? schemaFieldName;

    // Find field in parent type using the schema field name
    const fieldDef = parentType.fields.find((f) => f.name === schemaFieldName);
    if (!fieldDef) continue;

    // Filter by @restrictTarget
    if (fieldDef.restrictTargets && targetName) {
      if (!fieldDef.restrictTargets.includes(targetName)) {
        continue; // Skip this field for this target
      }
    }

    const queryField: QueryFieldSelection = {
      name: accessorName,
      schemaType: fieldDef.type,
      selections: [],
    };

    // Recursively resolve sub-selections
    if (fieldNode.selectionSet) {
      const innerTypeName = getNamedTypeFromRef(fieldDef.type);

      // Check if this field's type is a union type
      const unionType = schemaModel.unionTypes.get(innerTypeName);
      if (unionType) {
        // Process inline fragments for union types
        queryField.inlineFragments = [];
        for (const subSelection of fieldNode.selectionSet.selections) {
          if (subSelection.kind === "InlineFragment") {
            const fragment = subSelection as InlineFragmentNode;
            const fragmentTypeName = fragment.typeCondition?.name.value;
            if (!fragmentTypeName || !fragment.selectionSet) continue;

            const fragmentObjectType = schemaModel.objectTypes.get(fragmentTypeName);
            if (!fragmentObjectType) continue;

            queryField.inlineFragments.push({
              typeName: fragmentTypeName,
              selections: resolveSelections(
                fragment.selectionSet,
                fragmentObjectType,
                schemaModel,
                targetName
              ),
            });
          }
          // Skip Field selections within union fields (like __typename, already handled)
        }
      } else {
        const innerType = schemaModel.objectTypes.get(innerTypeName);
        if (innerType) {
          queryField.selections = resolveSelections(
            fieldNode.selectionSet,
            innerType,
            schemaModel,
            targetName
          );
        }
      }
    }

    result.push(queryField);
  }

  return result;
}

function getNamedTypeFromRef(typeRef: TypeRef): string {
  switch (typeRef.kind) {
    case "Named":
      return typeRef.name!;
    case "NonNull":
    case "List":
      return getNamedTypeFromRef(typeRef.ofType!);
  }
}

/**
 * Parse a supplementary GraphQL types file and merge its type definitions
 * into the schema model. This is used for --json-types to define shapes
 * for JSON scalar fields.
 */
export function mergeJsonTypes(
  jsonTypesSource: string,
  schemaModel: SchemaModel
): void {
  // Wrap in a minimal schema if needed to make it valid for buildSchema
  const wrappedSource = `type Query { _: String }\n${jsonTypesSource}`;
  const schema = buildSchema(wrappedSource);

  const typeMap = schema.getTypeMap();
  for (const [typeName, type] of Object.entries(typeMap)) {
    if (typeName.startsWith("__")) continue;
    if (["String", "Int", "Float", "Boolean", "ID", "Query"].includes(typeName))
      continue;

    if (isObjectType(type)) {
      const fields: FieldDefinition[] = [];
      const fieldMap = type.getFields();
      for (const [fieldName, field] of Object.entries(fieldMap)) {
        const fieldDef: FieldDefinition = {
          name: fieldName,
          type: graphqlTypeToTypeRef(field.type),
        };
        fields.push(fieldDef);
      }
      schemaModel.objectTypes.set(typeName, { name: typeName, fields });
    } else if (isEnumType(type)) {
      schemaModel.enumTypes.set(typeName, {
        name: typeName,
        values: type.getValues().map((v) => v.name),
      });
    }
  }
}

/**
 * Inject sub-selections for JSON scalar fields based on override mappings.
 * This post-processes the parsed query's selection tree, replacing scalar
 * JSON fields with typed sub-selections generated from the override type.
 */
export function injectJsonOverrides(
  selections: QueryFieldSelection[],
  overrides: Map<string, string>,
  schemaModel: SchemaModel,
  pathPrefix: string[] = []
): void {
  for (const selection of selections) {
    const currentPath = [...pathPrefix, selection.name];

    // Check if this field matches any override by suffix
    for (const [overridePath, typeName] of overrides) {
      if (pathSuffixMatches(currentPath, overridePath)) {
        // Only inject if the field currently has no sub-selections (it's a scalar)
        if (selection.selections.length === 0) {
          const overrideType = schemaModel.objectTypes.get(typeName);
          if (overrideType) {
            selection.selections = generateSelectionsFromType(
              overrideType,
              schemaModel
            );
          }
        }
      }
    }

    // Recurse into existing sub-selections
    if (selection.selections.length > 0) {
      injectJsonOverrides(
        selection.selections,
        overrides,
        schemaModel,
        currentPath
      );
    }

    // Recurse into inline fragments
    if (selection.inlineFragments) {
      for (const fragment of selection.inlineFragments) {
        injectJsonOverrides(
          fragment.selections,
          overrides,
          schemaModel,
          currentPath
        );
      }
    }
  }
}

/**
 * Check if a path suffix-matches an override key.
 * e.g., path ["metafield", "jsonValue"] matches override "jsonValue"
 * and also matches "metafield.jsonValue"
 */
function pathSuffixMatches(path: string[], overrideKey: string): boolean {
  const overrideParts = overrideKey.split(".");
  if (overrideParts.length > path.length) return false;

  for (let i = 0; i < overrideParts.length; i++) {
    if (path[path.length - overrideParts.length + i] !== overrideParts[i]) {
      return false;
    }
  }
  return true;
}

/**
 * Generate sub-selections for all fields of a type, recursively for nested objects.
 */
function generateSelectionsFromType(
  type: ObjectType,
  schemaModel: SchemaModel
): QueryFieldSelection[] {
  const selections: QueryFieldSelection[] = [];

  for (const field of type.fields) {
    const namedType = getNamedTypeFromRef(field.type);
    const nestedObjectType = schemaModel.objectTypes.get(namedType);

    const selection: QueryFieldSelection = {
      name: field.name,
      schemaType: field.type,
      selections: nestedObjectType
        ? generateSelectionsFromType(nestedObjectType, schemaModel)
        : [],
    };

    selections.push(selection);
  }

  return selections;
}
