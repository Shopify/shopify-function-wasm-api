#!/usr/bin/env node

/**
 * Shopify Function Codegen CLI
 *
 * Generates typed code for Shopify Function SDKs from GraphQL schemas and queries.
 *
 * Usage:
 *   shopify-function-codegen --language zig
 *
 * With explicit paths:
 *   shopify-function-codegen \
 *     --schema ./schema.graphql \
 *     --query ./a.graphql \
 *     --query ./b.graphql \
 *     --language zig \
 *     --output ./generated/ \
 *     --enums-as-str CountryCode,LanguageCode,CurrencyCode
 *
 * When --schema is omitted, defaults to "schema.graphql" in the current directory.
 * When --query is omitted, auto-discovers *.graphql files in the current directory
 * (excluding the schema file and any --json-types file).
 */

import * as fs from "node:fs";
import * as path from "node:path";
import {
  parseSchema,
  parseQuery,
  mergeJsonTypes,
  injectJsonOverrides,
} from "./parser.js";
import { emitZig, camelToSnake } from "./emitters/zig.js";
import { emitC } from "./emitters/c.js";
import { emitGo } from "./emitters/go.js";

interface QueryArg {
  path: string;
  target?: string; // explicit mutation target name (camelCase)
}

interface CliArgs {
  schema: string;
  queries: QueryArg[];
  language: string;
  output: string;
  enumsAsStr: string[];
  goModulePath: string;
  goPackage: string;
  jsonTypes: string;
  jsonOverrides: Map<string, string>;
}

function parseArgs(argv: string[]): CliArgs {
  const args: CliArgs = {
    schema: "",
    queries: [],
    language: "zig",
    output: "./generated/",
    enumsAsStr: ["LanguageCode", "CountryCode", "CurrencyCode"],
    goModulePath: "github.com/Shopify/shopify-function-go",
    goPackage: "generated",
    jsonTypes: "",
    jsonOverrides: new Map(),
  };

  let i = 0;
  while (i < argv.length) {
    switch (argv[i]) {
      case "--schema":
        args.schema = argv[++i];
        break;
      case "--query":
        args.queries.push({ path: argv[++i] });
        break;
      case "--target":
        // Pairs with the most recent --query to specify which mutation target it maps to
        if (args.queries.length === 0) {
          console.error("Error: --target must follow a --query");
          process.exit(1);
        }
        args.queries[args.queries.length - 1].target = argv[++i];
        break;
      case "--language":
        args.language = argv[++i];
        break;
      case "--output":
        args.output = argv[++i];
        break;
      case "--enums-as-str":
        args.enumsAsStr = argv[++i].split(",").map((s) => s.trim());
        break;
      case "--go-module-path":
        args.goModulePath = argv[++i];
        break;
      case "--go-package":
        args.goPackage = argv[++i];
        break;
      case "--json-types":
        args.jsonTypes = argv[++i];
        break;
      case "--json-override": {
        const val = argv[++i];
        const eqIndex = val.indexOf("=");
        if (eqIndex === -1) {
          console.error(
            `Error: --json-override value must be in format "fieldPath=TypeName", got "${val}"`
          );
          process.exit(1);
        }
        args.jsonOverrides.set(val.slice(0, eqIndex), val.slice(eqIndex + 1));
        break;
      }
      default:
        console.error(`Unknown option: ${argv[i]}`);
        process.exit(1);
    }
    i++;
  }

  // Default schema to schema.graphql in the current directory
  if (!args.schema) {
    const defaultSchema = "schema.graphql";
    if (fs.existsSync(defaultSchema)) {
      args.schema = defaultSchema;
    } else {
      console.error(
        "Error: --schema is required (no schema.graphql found in current directory)"
      );
      process.exit(1);
    }
  }

  // Auto-discover query files if none were explicitly provided
  if (args.queries.length === 0) {
    const schemaBasename = path.basename(args.schema);
    const jsonTypesBasename = args.jsonTypes
      ? path.basename(args.jsonTypes)
      : "";
    const excluded = new Set([schemaBasename, jsonTypesBasename].filter(Boolean));

    const discovered = fs
      .readdirSync(".")
      .filter((f) => f.endsWith(".graphql") && !excluded.has(f))
      .sort();

    if (discovered.length === 0) {
      console.error(
        "Error: at least one --query is required (no .graphql query files found in current directory)"
      );
      process.exit(1);
    }

    args.queries = discovered.map((f) => ({ path: f }));
  }

  return args;
}

function main() {
  const args = parseArgs(process.argv.slice(2));

  // Read and parse schema
  const schemaSource = fs.readFileSync(args.schema, "utf-8");
  const schemaModel = parseSchema(schemaSource);

  // Merge supplementary JSON type definitions if provided
  if (args.jsonTypes) {
    const jsonTypesSource = fs.readFileSync(args.jsonTypes, "utf-8");
    mergeJsonTypes(jsonTypesSource, schemaModel);
  }

  // Determine target names from mutation targets in schema
  // Each query file maps to a target either by explicit --target flag,
  // by matching query file name to mutation target name, or by index order
  const targets = args.queries.map((queryArg, index) => {
    const querySource = fs.readFileSync(queryArg.path, "utf-8");
    const queryFileName = path.basename(queryArg.path, ".graphql");

    let mutationTarget: (typeof schemaModel.mutationTargets)[number] | undefined;

    if (queryArg.target) {
      // Explicit --target flag: find mutation target by camelCase name
      mutationTarget = schemaModel.mutationTargets.find(
        (t) => t.name === queryArg.target
      );
      if (!mutationTarget) {
        console.error(
          `Error: mutation target "${queryArg.target}" not found in schema. Available: ${schemaModel.mutationTargets.map((t) => t.name).join(", ")}`
        );
        process.exit(1);
      }
    } else {
      // Try to match query file name to a mutation target by name
      // e.g., "cart_validations_generate_run" matches "cartValidationsGenerateRun"
      mutationTarget = schemaModel.mutationTargets.find(
        (t) => camelToSnake(t.name) === queryFileName
      );

      // Fall back to index-based matching
      if (!mutationTarget) {
        mutationTarget = schemaModel.mutationTargets[index];
      }
    }

    if (!mutationTarget) {
      console.error(
        `Error: no mutation target found for query file ${queryArg.path}. Available: ${schemaModel.mutationTargets.map((t) => t.name).join(", ")}`
      );
      process.exit(1);
    }

    // Build the target name from the mutation (e.g., "cartValidationsGenerateRun" -> "cart_validations_generate_run")
    const targetName = camelToSnake(mutationTarget.name);

    // Determine the full target identifier for @restrictTarget filtering
    // Convert snake_case target name to dot-separated format matching Shopify's convention
    // e.g., "cart_validations_generate_run" -> "cart.validations.generate.run"
    const targetFilterName = targetName.replace(/_/g, ".");

    const parsedQuery = parseQuery(querySource, schemaModel, targetFilterName);

    // Inject typed sub-selections for JSON override fields
    if (args.jsonOverrides.size > 0) {
      injectJsonOverrides(parsedQuery.selections, args.jsonOverrides, schemaModel);
    }

    return {
      targetName,
      graphqlTargetName: mutationTarget.name,
      selections: parsedQuery.selections,
    };
  });

  // Generate code and write output
  fs.mkdirSync(args.output, { recursive: true });

  switch (args.language) {
    case "zig": {
      const output = emitZig(schemaModel, targets, {
        enumsAsStr: args.enumsAsStr,
      });
      const outputPath = path.join(args.output, "schema.zig");
      fs.writeFileSync(outputPath, output);
      console.log(`Generated ${outputPath}`);
      break;
    }
    case "c": {
      const result = emitC(schemaModel, targets, {
        enumsAsStr: args.enumsAsStr,
      });
      const headerPath = path.join(args.output, "schema.h");
      const sourcePath = path.join(args.output, "schema.c");
      fs.writeFileSync(headerPath, result.header);
      fs.writeFileSync(sourcePath, result.source);
      console.log(`Generated ${headerPath}`);
      console.log(`Generated ${sourcePath}`);
      break;
    }
    case "go": {
      const output = emitGo(schemaModel, targets, {
        enumsAsStr: args.enumsAsStr,
        modulePath: args.goModulePath,
        packageName: args.goPackage,
      });
      const outputPath = path.join(args.output, "schema.go");
      fs.writeFileSync(outputPath, output);
      console.log(`Generated ${outputPath}`);
      break;
    }
    default:
      console.error(`Unsupported language: ${args.language}`);
      process.exit(1);
  }
}

main();
