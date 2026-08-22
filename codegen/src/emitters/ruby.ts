/**
 * Ruby code emitter.
 * Generates the three files the rubywat toolchain expects from a SchemaModel
 * and parsed queries:
 *
 *   - schema.rb      compiled helpers: input accessors and output constructors
 *   - schema.rbs     inline-RBS interfaces describing the receiver surface
 *   - schema_lsp.rb  plain Ruby stubs so editors resolve the same surface
 *
 * rubywat lowers a receiver call like `input.cart` to `cart(input)`, so every
 * selected field becomes a one-parameter function whose inline RBS signature
 * names the JSON record it reads from. Output objects are plain hashes built by
 * `__Type_new` (or `__Type_variant` for GraphQL `@oneOf` inputs) helpers.
 */

import {
  SchemaModel,
  InputObjectType,
  FieldDefinition,
  TypeRef,
  getNamedType,
  unwrapNonNull,
  nonNull,
  named,
} from "../schema-model.js";
import { QueryFieldSelection } from "../parser.js";
import { camelToSnake } from "./zig.js";

export interface RubyEmitterOptions {
  /**
   * Accepted for parity with the other emitters. Ruby models every GraphQL enum
   * as a String, so no enum types are generated regardless of this list.
   */
  enumsAsStr: string[];
}

export interface RubyTargetQuery {
  targetName: string; // e.g., "cart_validations_generate_run" (snake_case)
  graphqlTargetName: string; // e.g., "cartValidationsGenerateRun" (camelCase)
  resultTypeName?: string; // e.g., "CartValidationsGenerateRunResult"
  selections: QueryFieldSelection[];
}

export interface RubyEmitterOutput {
  ruby: string; // schema.rb
  rbs: string; // schema.rbs
  rbi: string; // schema_lsp.rb
}

/** Comments wrap greedily at this column, including indent and the "# " prefix. */
const COMMENT_WIDTH = 90;

const RUBY_KEYWORDS = new Set([
  "BEGIN",
  "END",
  "alias",
  "and",
  "begin",
  "break",
  "case",
  "class",
  "def",
  "defined?",
  "do",
  "else",
  "elsif",
  "end",
  "ensure",
  "false",
  "for",
  "if",
  "in",
  "module",
  "next",
  "nil",
  "not",
  "or",
  "redo",
  "rescue",
  "retry",
  "return",
  "self",
  "super",
  "then",
  "true",
  "undef",
  "unless",
  "until",
  "when",
  "while",
  "yield",
]);

/** A selection, optionally carrying a description supplied by the caller. */
type RubySelection = QueryFieldSelection & { description?: string };

interface ReceiverField {
  method: string;
  jsonKey: string;
  rbsType: string;
  docType: string;
  stub: string;
  description?: string;
}

interface ReceiverType {
  name: string; // record name without the RBS interface underscore
  fields: ReceiverField[];
}

/** One generated accessor, which may serve several receiver records. */
interface Accessor {
  method: string;
  jsonKey: string;
  entries: Array<{ receiver: string; returnType: string }>;
}

interface ReceiverModel {
  receivers: ReceiverType[];
  accessors: Accessor[];
}

interface OutputParam {
  name: string; // Ruby keyword argument
  jsonKey: string; // GraphQL field name written into the hash
  rbsType: string;
}

interface OutputConstructor {
  helper: string; // e.g., "__Operation_validation_add"
  method: string; // e.g., "validation_add"
  variantKey?: string; // GraphQL field name of a @oneOf variant
  inlined: boolean; // variant payload fields are inlined into the wrapper hash
  params: OutputParam[];
}

interface OutputType {
  name: string;
  isOneOf: boolean;
  fieldNames: string[];
  constructors: OutputConstructor[];
}

/**
 * Generate schema.rb, schema.rbs, and schema_lsp.rb from a schema model and
 * queries.
 */
export function emitRuby(
  schema: SchemaModel,
  targets: RubyTargetQuery[],
  options: RubyEmitterOptions = { enumsAsStr: [] }
): RubyEmitterOutput {
  const model = buildReceivers(schema, targets);
  const outputs = buildOutputTypes(schema, targets);

  return {
    ruby: emitSchemaRb(model, outputs),
    rbs: emitSchemaRbs(model, outputs),
    rbi: emitSchemaLsp(model, outputs),
  };
}

// === Receiver records ===

function buildReceivers(
  schema: SchemaModel,
  targets: RubyTargetQuery[]
): ReceiverModel {
  const receivers: ReceiverType[] = [];
  const visited = new Set<string>();
  const accessors: Accessor[] = [];
  const accessorIndex = new Map<string, number>();
  const methodNames = new Map<string, string>();
  const usedMethods = new Set<string>();

  // The same response key always lowers to the same Ruby method, so accessors
  // shared by several records merge instead of colliding. Distinct keys that
  // snake_case to the same name get a numeric suffix.
  function methodFor(responseKey: string): string {
    const cached = methodNames.get(responseKey);
    if (cached) return cached;

    const base = rubyMethodName(responseKey);
    let name = base;
    let suffix = 2;
    while (usedMethods.has(name)) {
      name = `${base}_${suffix++}`;
    }
    usedMethods.add(name);
    methodNames.set(responseKey, name);
    return name;
  }

  function recordAccessor(
    method: string,
    jsonKey: string,
    receiver: string,
    returnType: string
  ) {
    const at = accessorIndex.get(method);
    if (at === undefined) {
      accessorIndex.set(method, accessors.length);
      accessors.push({
        method,
        jsonKey,
        entries: [{ receiver, returnType }],
      });
      return;
    }
    const accessor = accessors[at];
    if (accessor.entries.some((entry) => entry.receiver === receiver)) return;
    accessor.entries.push({ receiver, returnType });
  }

  function visit(recordName: string, selections: RubySelection[]) {
    if (visited.has(recordName)) return;
    visited.add(recordName);

    const receiver: ReceiverType = { name: recordName, fields: [] };
    receivers.push(receiver);
    const children: Array<{ name: string; selections: RubySelection[] }> = [];

    for (const selection of selections) {
      const childSelections = mergeChildSelections(selection, schema);
      let rbsLeaf: string;
      let docLeaf: string;
      let leafStub: string;

      if (childSelections.length > 0) {
        const childName = `${recordName}${pascalCase(selection.name)}${"Item".repeat(listDepth(selection.schemaType))}`;
        rbsLeaf = `_${childName}`;
        docLeaf = childName;
        leafStub = `${childName}.new`;
        children.push({ name: childName, selections: childSelections });
      } else {
        const leaf = scalarLeaf(getNamedType(selection.schemaType));
        rbsLeaf = leaf.type;
        docLeaf = leaf.type;
        leafStub = leaf.stub;
      }

      const field: ReceiverField = {
        method: methodFor(selection.name),
        jsonKey: selection.name,
        rbsType: rbsType(selection.schemaType, rbsLeaf),
        docType: docType(selection.schemaType, docLeaf),
        stub: stubFor(selection.schemaType, leafStub),
        description: selection.description,
      };
      receiver.fields.push(field);
      recordAccessor(field.method, field.jsonKey, recordName, field.rbsType);
    }

    for (const child of children) {
      visit(child.name, child.selections);
    }
  }

  for (const target of targets) {
    visit(
      `${pascalCase(target.graphqlTargetName)}Input`,
      target.selections as RubySelection[]
    );
  }

  return { receivers, accessors };
}

/**
 * Flatten a field's sub-selections. Union fields expose one receiver surface
 * holding every variant field, plus the `__typename` discriminator, because the
 * concrete variant is only known at runtime. Fields that some union member does
 * not carry become nullable.
 */
function mergeChildSelections(
  selection: RubySelection,
  schema: SchemaModel
): RubySelection[] {
  const fragments = selection.inlineFragments ?? [];
  if (fragments.length === 0) return selection.selections;

  const merged: RubySelection[] = [];
  const positions = new Map<string, number>();
  const occurrences = new Map<string, number>();

  function push(child: RubySelection) {
    positions.set(child.name, merged.length);
    merged.push(child);
  }

  if (!selection.selections.some((child) => child.name === "__typename")) {
    push({
      name: "__typename",
      schemaType: nonNull(named("String")),
      selections: [],
    });
  }
  for (const child of selection.selections) {
    push(child);
  }

  for (const fragment of fragments) {
    for (const child of fragment.selections) {
      occurrences.set(child.name, (occurrences.get(child.name) ?? 0) + 1);
      const at = positions.get(child.name);
      if (at === undefined) {
        push(child);
      } else {
        merged[at] = mergeSelection(merged[at], child);
      }
    }
  }

  const memberCount =
    schema.unionTypes.get(getNamedType(selection.schemaType))?.memberTypes
      .length ?? fragments.length;

  return merged.map((child) => {
    const seen = occurrences.get(child.name) ?? 0;
    if (seen === 0 || seen >= memberCount) return child;
    return { ...child, schemaType: unwrapNonNull(child.schemaType) };
  });
}

/** Union two selections of the same response key, field by field. */
function mergeSelection(
  existing: RubySelection,
  incoming: RubySelection
): RubySelection {
  const inlineFragments = [
    ...(existing.inlineFragments ?? []),
    ...(incoming.inlineFragments ?? []),
  ];
  if (incoming.selections.length === 0 && inlineFragments.length === 0) {
    return existing;
  }

  const selections = [...existing.selections];
  for (const child of incoming.selections) {
    const at = selections.findIndex((candidate) => candidate.name === child.name);
    if (at === -1) {
      selections.push(child);
    } else {
      selections[at] = mergeSelection(selections[at], child);
    }
  }

  return {
    ...existing,
    selections,
    inlineFragments: inlineFragments.length > 0 ? inlineFragments : undefined,
  };
}

// === Output objects ===

function buildOutputTypes(
  schema: SchemaModel,
  targets: RubyTargetQuery[]
): OutputType[] {
  const outputs: OutputType[] = [];
  const visited = new Set<string>();

  function visit(typeName: string) {
    if (visited.has(typeName)) return;
    const inputType = schema.inputTypes.get(typeName);
    if (!inputType) return;

    visited.add(typeName);
    outputs.push(buildOutputType(inputType, schema));
    for (const field of inputType.fields) {
      visit(getNamedType(field.type));
    }
  }

  for (const target of targets) {
    const resultTypeName =
      target.resultTypeName ??
      schema.mutationTargets.find(
        (candidate) => candidate.name === target.graphqlTargetName
      )?.resultTypeName;
    if (resultTypeName) visit(resultTypeName);
  }

  return outputs;
}

function buildOutputType(
  inputType: InputObjectType,
  schema: SchemaModel
): OutputType {
  const fieldNames = inputType.fields.map((field) => field.name);

  if (!inputType.isOneOf) {
    return {
      name: inputType.name,
      isOneOf: false,
      fieldNames,
      constructors: [
        {
          helper: `__${inputType.name}_new`,
          method: "new",
          inlined: false,
          params: inputType.fields.map((field) => outputParam(field, schema)),
        },
      ],
    };
  }

  // A @oneOf input is a tagged union: one constructor per variant, each wrapping
  // its payload under the variant key.
  const constructors = inputType.fields.map((field) => {
    const payload = schema.inputTypes.get(getNamedType(field.type));
    const inlined =
      payload !== undefined &&
      !payload.isOneOf &&
      payload.fields.length > 0 &&
      unwrapNonNull(field.type).kind !== "List";
    const method = rubyMethodName(field.name);
    return {
      helper: `__${inputType.name}_${method}`,
      method,
      variantKey: field.name,
      inlined,
      params: inlined
        ? payload!.fields.map((payloadField) => outputParam(payloadField, schema))
        : [outputParam(field, schema)],
    };
  });

  return { name: inputType.name, isOneOf: true, fieldNames, constructors };
}

function outputParam(field: FieldDefinition, schema: SchemaModel): OutputParam {
  const leafName = getNamedType(field.type);
  const leaf = schema.inputTypes.has(leafName)
    ? leafName
    : scalarLeaf(leafName).type;
  return {
    name: rubyMethodName(field.name),
    jsonKey: field.name,
    rbsType: rbsType(field.type, leaf),
  };
}

/**
 * Format an accessor's receiver and return types. An accessor shared by several
 * records takes a union of receivers; rubywat correlates receiver position N
 * with return position N, so the return union keeps one entry per receiver
 * unless every receiver returns the same type.
 */
function accessorSignature(accessor: Accessor): {
  receiver: string;
  returnType: string;
} {
  const returnTypes = accessor.entries.map((entry) => entry.returnType);
  const distinctReturns = [...new Set(returnTypes)];
  return {
    receiver: accessor.entries.map((entry) => `_${entry.receiver}`).join(" | "),
    returnType:
      distinctReturns.length === 1
        ? distinctReturns[0]
        : `(${returnTypes.join(" | ")})`,
  };
}

// === schema.rb ===

function emitSchemaRb(model: ReceiverModel, outputs: OutputType[]): string {
  const lines: string[] = [
    "# frozen_string_literal: true",
    "",
    "# Generated by shopify-function-codegen --language ruby.",
    "# rubywat lowers receiver calls like input.cart to cart(input).",
    "",
    "private",
    "",
  ];

  for (const accessor of model.accessors) {
    const { receiver, returnType } = accessorSignature(accessor);
    const param =
      accessor.entries.length === 1
        ? rubyIdentifier(camelToSnake(accessor.entries[0].receiver))
        : "receiver";

    lines.push(`#: (${receiver}) -> ${returnType}`);
    lines.push(`def ${accessor.method}(${param})`);
    lines.push(`  ${param}["${accessor.jsonKey}"]`);
    lines.push("end");
    lines.push("");
  }

  if (outputs.length > 0) {
    lines.push("# Output type helpers used by rubywat lowering.");
    lines.push("");
    for (const output of outputs) {
      for (const constructor of output.constructors) {
        lines.push(...emitOutputHelper(output, constructor));
        lines.push("");
      }
    }
  }

  while (lines[lines.length - 1] === "") lines.pop();
  return `${lines.join("\n")}\n`;
}

function emitOutputHelper(
  output: OutputType,
  constructor: OutputConstructor
): string[] {
  const lines: string[] = [];
  const signature = constructor.params.map((param) => param.rbsType).join(", ");
  const paramList = constructor.params.map((param) => param.name).join(", ");

  lines.push(`#: (${signature}) -> ${output.name}`);
  lines.push(
    paramList ? `def ${constructor.helper}(${paramList})` : `def ${constructor.helper}`
  );

  if (constructor.params.length === 0) {
    lines.push("  {}");
  } else if (constructor.variantKey && constructor.inlined) {
    lines.push("  {");
    lines.push(`    ${constructor.variantKey}: {`);
    lines.push(...hashEntries(constructor.params, "      "));
    lines.push("    }");
    lines.push("  }");
  } else if (constructor.variantKey) {
    lines.push("  {");
    lines.push(`    ${constructor.variantKey}: ${constructor.params[0].name}`);
    lines.push("  }");
  } else {
    lines.push("  {");
    lines.push(...hashEntries(constructor.params, "    "));
    lines.push("  }");
  }

  lines.push("end");
  return lines;
}

function hashEntries(params: OutputParam[], indent: string): string[] {
  return params.map(
    (param, index) =>
      `${indent}${param.jsonKey}: ${param.name}${index === params.length - 1 ? "" : ","}`
  );
}

// === schema.rbs ===

function emitSchemaRbs(model: ReceiverModel, outputs: OutputType[]): string {
  const lines: string[] = [
    "# Generated by shopify-function-codegen --language ruby.",
    "# Interfaces describe the receiver-method surface available to Ruby LSP.",
    "",
  ];

  for (const receiver of model.receivers) {
    lines.push(...comment(receiverDoc(receiver), ""));
    lines.push(`interface _${receiver.name}`);
    for (const field of receiver.fields) {
      lines.push(...comment(fieldDoc(receiver, field, field.rbsType), "  "));
      lines.push(`  def ${field.method}: () -> ${field.rbsType}`);
    }
    lines.push("end");
    lines.push("");
  }

  if (model.accessors.length > 0) {
    // rubywat resolves `receiver.field` through these top-level helpers.
    lines.push("class Object");
    for (const accessor of model.accessors) {
      const { receiver, returnType } = accessorSignature(accessor);
      lines.push(`  def ${accessor.method}: (${receiver}) -> ${returnType}`);
    }
    lines.push("end");
    lines.push("");
  }

  for (const output of outputs) {
    lines.push(...comment(outputDoc(output), ""));
    lines.push(`class ${output.name}`);
    for (const constructor of output.constructors) {
      const params = constructor.params
        .map((param) => `${param.name}: ${param.rbsType}`)
        .join(", ");
      lines.push(...comment(constructorDoc(output, constructor), "  "));
      lines.push(
        `  def self.${constructor.method}: (${params}) -> ${output.name}`
      );
    }
    lines.push("end");
    lines.push("");
  }

  return `${lines.join("\n")}\n`;
}

// === schema_lsp.rb ===

function emitSchemaLsp(model: ReceiverModel, outputs: OutputType[]): string {
  const lines: string[] = [
    "# frozen_string_literal: true",
    "",
    "# Generated by shopify-function-codegen --language ruby.",
    "# Ruby LSP stubs only. Do not compile this file with rubywat.",
    "",
  ];

  for (const receiver of model.receivers) {
    lines.push(...comment(receiverDoc(receiver), ""));
    lines.push(`class ${receiver.name}`);
    receiver.fields.forEach((field, index) => {
      if (index > 0) lines.push("");
      lines.push(...comment(fieldDoc(receiver, field, field.docType), "  "));
      lines.push(`  def ${field.method}`);
      lines.push(`    ${field.stub}`);
      lines.push("  end");
    });
    lines.push("end");
    lines.push("");
  }

  for (const output of outputs) {
    lines.push(...comment(outputDoc(output), ""));
    lines.push(`class ${output.name}`);
    output.constructors.forEach((constructor, index) => {
      if (index > 0) lines.push("");
      const params = constructor.params.map((param) => `${param.name}:`).join(", ");
      lines.push(...comment(constructorDoc(output, constructor), "  "));
      lines.push(
        params
          ? `  def self.${constructor.method}(${params})`
          : `  def self.${constructor.method}`
      );
      lines.push("    {}");
      lines.push("  end");
    });
    lines.push("end");
    lines.push("");
  }

  return `${lines.join("\n")}\n`;
}

// === Documentation text ===

function receiverDoc(receiver: ReceiverType): string {
  return `Input object selected by GraphQL as ${receiver.name}.`;
}

function fieldDoc(
  receiver: ReceiverType,
  field: ReceiverField,
  type: string
): string {
  const sentence = `Reads the ${field.jsonKey} field from ${receiver.name}. Returns ${type}.`;
  const description = field.description?.replace(/\s+/g, " ").trim();
  return description ? `${sentence} ${description}` : sentence;
}

function outputDoc(output: OutputType): string {
  if (output.isOneOf) {
    return `${output.name} is a GraphQL @oneOf output union. Use exactly one variant constructor. Variants: ${list(output.fieldNames, ", ")}.`;
  }
  return `${output.name} is a generated function output object. Required fields: ${list(output.fieldNames, ", ")}.`;
}

function constructorDoc(
  output: OutputType,
  constructor: OutputConstructor
): string {
  const fields = list(
    constructor.params.map((param) => param.name),
    "; "
  );
  if (output.isOneOf) {
    return `Builds the ${output.name}.${constructor.method} variant. Fields: ${fields}.`;
  }
  return `Builds a ${output.name} output object. Fields: ${fields}.`;
}

function list(items: string[], separator: string): string {
  return items.length > 0 ? items.join(separator) : "none";
}

/** Wrap a sentence into Ruby comment lines that fit COMMENT_WIDTH. */
function comment(text: string, indent: string): string[] {
  const prefix = `${indent}# `;
  const words = text.split(/\s+/).filter(Boolean);
  const lines: string[] = [];
  let current = "";

  for (const word of words) {
    if (!current) {
      current = word;
    } else if (`${prefix}${current} ${word}`.length > COMMENT_WIDTH) {
      lines.push(`${prefix}${current}`);
      current = word;
    } else {
      current = `${current} ${word}`;
    }
  }
  if (current) lines.push(`${prefix}${current}`);

  return lines.length > 0 ? lines : [`${indent}#`];
}

// === Type formatting ===

function rbsType(type: TypeRef, leaf: string): string {
  if (type.kind === "NonNull") return rbsCore(type.ofType!, leaf);
  return `${rbsCore(type, leaf)}?`;
}

function rbsCore(type: TypeRef, leaf: string): string {
  if (type.kind === "NonNull") return rbsCore(type.ofType!, leaf);
  if (type.kind === "List") return `Array[${rbsType(type.ofType!, leaf)}]`;
  return leaf;
}

/** Ruby LSP documentation uses `Array<T>` and a trailing `, nil` for nullables. */
function docType(type: TypeRef, leaf: string): string {
  if (type.kind === "NonNull") return docCore(type.ofType!, leaf);
  return `${docCore(type, leaf)}, nil`;
}

function docCore(type: TypeRef, leaf: string): string {
  if (type.kind === "NonNull") return docCore(type.ofType!, leaf);
  if (type.kind === "List") return `Array<${docCore(type.ofType!, leaf)}>`;
  return leaf;
}

function stubFor(type: TypeRef, leafStub: string): string {
  return unwrapNonNull(type).kind === "List" ? "[]" : leafStub;
}

function listDepth(type: TypeRef): number {
  if (type.kind === "NonNull") return listDepth(type.ofType!);
  if (type.kind === "List") return 1 + listDepth(type.ofType!);
  return 0;
}

/**
 * Scalars and enums both arrive as JSON leaves. Enums are strings in Ruby, so
 * anything that is not a selected object type falls back to a string.
 */
function scalarLeaf(typeName: string): { type: string; stub: string } {
  switch (typeName) {
    case "Int":
      return { type: "Integer", stub: "0" };
    case "Float":
      return { type: "Float", stub: "0.0" };
    case "Boolean":
      return { type: "bool", stub: "false" };
    case "Json":
    case "JSON":
      return { type: "untyped", stub: "nil" };
    case "Void":
      return { type: "nil", stub: "nil" };
    default:
      return { type: "String", stub: '""' };
  }
}

// === Naming ===

function pascalCase(name: string): string {
  return name
    .split(/[^A-Za-z0-9]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join("");
}

/**
 * snake_case a GraphQL response key, preserving leading underscores so
 * `__typename` keeps its shape.
 */
function rubyMethodName(name: string): string {
  const leadingUnderscores = name.match(/^_*/)![0];
  return rubyIdentifier(
    `${leadingUnderscores}${camelToSnake(name.slice(leadingUnderscores.length))}`
  );
}

function rubyIdentifier(name: string): string {
  return RUBY_KEYWORDS.has(name) ? `${name}_` : name;
}
