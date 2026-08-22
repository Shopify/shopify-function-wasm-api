# Shopify Function Codegen

Generates typed SDK code from GraphQL schemas and queries for Zig, C, Go, and Ruby.

## Setup

```bash
cd codegen
npm install
npx tsc
```

## Usage

```bash
node dist/src/index.js \
  --schema ./schema.graphql \
  --query ./a.graphql \
  --query ./b.graphql \
  --language zig \
  --output ./generated/ \
  --enums-as-str CountryCode,LanguageCode,CurrencyCode
```

### Languages

**Zig** (`--language zig`): outputs `schema.zig`

**C** (`--language c`): outputs `schema.h` (types, forward declarations, accessor prototypes) and `schema.c` (serialization and accessor implementations)

**Go** (`--language go`): outputs `schema.go`
- `--go-module-path` — Go module path for the sf import (default: `github.com/Shopify/shopify-function-go`)
- `--go-package` — package name in generated file (default: `generated`)

**Ruby** (`--language ruby`): outputs `schema.rb`, `schema.rbs`, and `schema_lsp.rb`

Ruby functions are compiled by rubywat, which lowers a receiver call like `input.cart` into the top-level call `cart(input)`. The three files follow from that:

- `schema.rb` — one single-parameter method per selected field, each annotated with an inline RBS comment (`#: (_RunInput) -> String`) and returning `receiver["graphqlKey"]`. Output objects get `__Type_new` helpers (or one `__Type_<variant>` per variant for `@oneOf` inputs) that build the hash rubywat serializes. Compile this file with your function.
- `schema.rbs` — interfaces describing each receiver's method surface, plus a `class Object` block declaring the lowered one-argument form so type checkers accept both spellings.
- `schema_lsp.rb` — stub classes with the same methods and doc comments, for editors that index Ruby rather than RBS. Not meant to be compiled.

A method selected on more than one receiver takes a union of receivers, and the return types stay positional (`(String | String? | String)`) so rubywat can correlate receiver N with return N. Enums are always emitted as `String`, so `--enums-as-str` has no effect on Ruby output.

### Options

| Flag | Description |
|------|-------------|
| `--schema` | Path to GraphQL schema file (required) |
| `--query` | Path to query file, one per target (required, repeatable) |
| `--target` | Mutation field for the preceding query, when it cannot be inferred from its filename |
| `--target-handle` | Exact API handle used by `@restrictTarget` for the preceding query |
| `--language` | Target language: `zig`, `c`, `go`, or `ruby` |
| `--output` | Output directory (default: `./generated/`) |
| `--enums-as-str` | Comma-separated enum types to treat as strings (default: `LanguageCode,CountryCode,CurrencyCode`) |
| `--json-types` | Path to a GraphQL file defining types for JSON scalar fields |
| `--json-override` | Map a JSON field to a type: `fieldPath=TypeName` (repeatable) |

`@restrictTarget` values are API handles and are not inferred from GraphQL mutation names. Pass `--target-handle` when filtering is required; otherwise the selections explicitly present in each query are generated unchanged.

### Typed JSON values

JSON scalar fields (e.g. `Metafield.jsonValue`) are untyped by default. You can provide type definitions and map them onto specific fields to get typed accessors:

1. Define types in a separate GraphQL file:

```graphql
# config_types.graphql
type Configuration {
  maxQuantity: Int!
  message: String!
}
```

2. Pass `--json-types` and `--json-override` to the codegen:

```bash
npx @shopify/shopify-function-codegen \
  --schema schema.graphql \
  --query run.graphql \
  --language zig \
  --output ./generated/ \
  --json-types config_types.graphql \
  --json-override "jsonValue=Configuration"
```

The override key uses suffix matching — `jsonValue` matches any field path ending with `jsonValue` (e.g. `metafield.jsonValue`, `cart.metafield.jsonValue`). For more specific matching, use a dotted path like `metafield.jsonValue`.

The generated code will include typed accessors for the JSON field's sub-fields (e.g. `.maxQuantity()`, `.message()`) instead of returning a raw JSON value.

## What it generates

- **Output types** — structs from GraphQL `input` types with serialization code
- **@oneOf unions** — tagged unions (Zig: `union(enum)`, C: enum tag + union, Go: interface + variants, Ruby: one constructor per variant)
- **Enums** — with `fromStr`/`toStr` conversion (unless in `--enums-as-str`)
- **Per-query Input types** — lazy accessors wrapping a raw `Value`, filtered by `@restrictTarget`
- **String interning** — field name lookups use interned string IDs for performance

Ruby is the exception to the last two: rubywat reads decoded JSON, so accessors are plain hash lookups with no wrapper types or interning.

## Tests

```bash
npx tsc && node --test dist/tests/*.test.js
```
