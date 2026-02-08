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
  isObjectType,
  isInputObjectType,
  isEnumType,
  isScalarType,
  isNonNullType,
  isListType,
  parse,
  DocumentNode,
  SelectionSetNode,
  FieldNode,
} from "graphql";

import {
  SchemaModel,
  ObjectType,
  InputObjectType,
  EnumType,
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
 * Represents a field selection from a query, with its resolved schema type.
 */
export interface QueryFieldSelection {
  name: string; // field name as it appears in the query
  schemaType: TypeRef; // type from the schema
  selections: QueryFieldSelection[]; // sub-selections for object fields
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
    const fieldName = fieldNode.name.value;

    // Find field in parent type
    const fieldDef = parentType.fields.find((f) => f.name === fieldName);
    if (!fieldDef) continue;

    // Filter by @restrictTarget
    if (fieldDef.restrictTargets && targetName) {
      if (!fieldDef.restrictTargets.includes(targetName)) {
        continue; // Skip this field for this target
      }
    }

    const queryField: QueryFieldSelection = {
      name: fieldName,
      schemaType: fieldDef.type,
      selections: [],
    };

    // Recursively resolve sub-selections
    if (fieldNode.selectionSet) {
      const innerTypeName = getNamedTypeFromRef(fieldDef.type);
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
