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
