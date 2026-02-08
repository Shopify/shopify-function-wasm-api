#!/usr/bin/env node

/**
 * Shopify Function Codegen CLI
 *
 * Generates typed code for Shopify Function SDKs from GraphQL schemas and queries.
 *
 * Usage:
 *   shopify-function-codegen \
 *     --schema ./schema.graphql \
 *     --query ./a.graphql \
 *     --query ./b.graphql \
 *     --language zig \
 *     --output ./generated/ \
 *     --enums-as-str CountryCode,LanguageCode,CurrencyCode
 */

import * as fs from "node:fs";
import * as path from "node:path";
import { parseSchema, parseQuery } from "./parser.js";
import { emitZig, camelToSnake } from "./emitters/zig.js";
import { emitC } from "./emitters/c.js";
import { emitGo } from "./emitters/go.js";

interface CliArgs {
  schema: string;
  queries: string[];
  language: string;
  output: string;
  enumsAsStr: string[];
  goModulePath: string;
  goPackage: string;
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
  };

  let i = 0;
  while (i < argv.length) {
    switch (argv[i]) {
      case "--schema":
        args.schema = argv[++i];
        break;
      case "--query":
        args.queries.push(argv[++i]);
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
      default:
        console.error(`Unknown option: ${argv[i]}`);
        process.exit(1);
    }
    i++;
  }

  if (!args.schema) {
    console.error("Error: --schema is required");
    process.exit(1);
  }

  if (args.queries.length === 0) {
    console.error("Error: at least one --query is required");
    process.exit(1);
  }

  return args;
}

function main() {
  const args = parseArgs(process.argv.slice(2));

  // Read and parse schema
  const schemaSource = fs.readFileSync(args.schema, "utf-8");
  const schemaModel = parseSchema(schemaSource);

  // Determine target names from mutation targets in schema
  // Each query file maps to a target based on order and mutation target names
  const targets = args.queries.map((queryPath, index) => {
    const querySource = fs.readFileSync(queryPath, "utf-8");
    const queryFileName = path.basename(queryPath, ".graphql");

    // Match query file to a mutation target
    const mutationTarget = schemaModel.mutationTargets[index];
    if (!mutationTarget) {
      console.error(
        `Error: no mutation target found for query file ${queryPath} (index ${index})`
      );
      process.exit(1);
    }

    // Build the target name from the mutation (e.g., "targetA" -> "target_a")
    const targetName = camelToSnake(mutationTarget.name);

    // Determine the full target identifier for @restrictTarget filtering
    // In the schema, @restrictTarget uses names like "test.target-b"
    // We need to find which targets each field is restricted to
    const targetFilterName = `test.${targetName.replace(/_/g, "-")}`;

    const parsedQuery = parseQuery(querySource, schemaModel, targetFilterName);

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
