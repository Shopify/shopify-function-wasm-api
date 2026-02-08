# Shopify Function Codegen

Generates typed SDK code from GraphQL schemas and queries for Zig, C, and Go.

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

### Options

| Flag | Description |
|------|-------------|
| `--schema` | Path to GraphQL schema file (required) |
| `--query` | Path to query file, one per target (required, repeatable) |
| `--language` | Target language: `zig`, `c`, or `go` |
| `--output` | Output directory (default: `./generated/`) |
| `--enums-as-str` | Comma-separated enum types to treat as strings (default: `LanguageCode,CountryCode,CurrencyCode`) |
| `--json-types` | Path to a GraphQL file defining types for JSON scalar fields |
| `--json-override` | Map a JSON field to a type: `fieldPath=TypeName` (repeatable) |

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
npx shopify-function-codegen \
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
- **@oneOf unions** — tagged unions (Zig: `union(enum)`, C: enum tag + union, Go: interface + variants)
- **Enums** — with `fromStr`/`toStr` conversion (unless in `--enums-as-str`)
- **Per-query Input types** — lazy accessors wrapping a raw `Value`, filtered by `@restrictTarget`
- **String interning** — field name lookups use interned string IDs for performance

## Tests

```bash
npx tsc && node --test dist/tests/codegen.test.js
```
