# Releasing codegen

The language SDKs and code generator are developed across independent repositories and released independently.

| Component | Tag format | Published artifact |
| --- | --- | --- |
| Codegen | `codegen/vX.Y.Z` | `@shopify/shopify-function-codegen` on npm |
| Go SDK | `vX.Y.Z` in `Shopify/shopify-function-go` | Go module |
| C SDK | `vX.Y.Z` in `Shopify/shopify-function-c` | GitHub release source archive |
| Zig SDK | `vX.Y.Z` in `Shopify/shopify-function-zig` | Zig package |

Before creating a codegen release, update `codegen/package.json` and create a `codegen/vX.Y.Z` release in this repository.

Publishing the release triggers the codegen workflow, which verifies the tag, runs the complete test suite, inspects the package, and publishes it to npm.

The npm workflow uses npm trusted publishing through GitHub Actions OIDC. Configure `@shopify/shopify-function-codegen` on npmjs.com to trust `.github/workflows/publish-codegen.yml` in this repository before the first release.

When generated code begins using a new SDK API, release the affected language SDK first, update the emitter and its minimum-version documentation, and then release codegen. Additive ABI changes can be adopted independently. Breaking ABI changes require a new versioned host module such as `shopify_function_v3`.
