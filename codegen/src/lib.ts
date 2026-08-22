/**
 * Library entry point for browser/programmatic use.
 *
 * Re-exports the pure functions from the codegen pipeline.
 * These have zero Node.js dependencies — only the `graphql` npm package.
 */

export {
  parseSchema,
  parseQuery,
  mergeJsonTypes,
  injectJsonOverrides,
} from "./parser.js";

export type {
  QueryFieldSelection,
  InlineFragmentSelection,
  ParsedQuery,
} from "./parser.js";

export { emitZig, camelToSnake } from "./emitters/zig.js";
export type { ZigEmitterOptions, TargetQuery } from "./emitters/zig.js";

export { emitC } from "./emitters/c.js";
export type { CEmitterOptions } from "./emitters/c.js";

export { emitGo } from "./emitters/go.js";
export type { GoEmitterOptions } from "./emitters/go.js";

export { emitRuby } from "./emitters/ruby.js";
export type {
  RubyEmitterOptions,
  RubyEmitterOutput,
  RubyTargetQuery,
} from "./emitters/ruby.js";

export type {
  SchemaModel,
  ObjectType,
  InputObjectType,
  EnumType,
  UnionType,
  MutationTarget,
  FieldDefinition,
  TypeRef,
} from "./schema-model.js";

export {
  nonNull,
  listOf,
  named,
  unwrapNonNull,
  isNullable,
  getNamedType,
  isBuiltinScalar,
  isCustomScalar,
  isScalar,
} from "./schema-model.js";
