import { test } from "node:test";
import * as assert from "node:assert/strict";
import { parseQuery, parseSchema } from "../src/parser.js";
import { emitC } from "../src/emitters/c.js";
import { emitGo } from "../src/emitters/go.js";
import { emitZig } from "../src/emitters/zig.js";
import { emitRuby } from "../src/emitters/ruby.js";

const DEFAULT_SEED = 0x5f3759df;
const DEFAULT_RUNS = 200;

function readPositiveInteger(name: string, fallback: number): number {
  const value = process.env[name];
  if (value === undefined) return fallback;

  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new Error(`${name} must be a positive integer, got ${value}`);
  }
  return parsed;
}

function mulberry32(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state += 0x6d2b79f5;
    let value = state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return ((value ^ (value >>> 14)) >>> 0) / 0x1_0000_0000;
  };
}

function pick<T>(random: () => number, values: readonly T[]): T {
  return values[Math.floor(random() * values.length)];
}

function maybeList(random: () => number, type: string): string {
  return pick(random, [type, `${type}!`, `[${type}!]`, `[${type}!]!`]);
}

function assertSaneOutput(language: string, output: string): void {
  assert.ok(output.length > 100, `${language} output was unexpectedly short`);
  assert.ok(!output.includes("undefined"), `${language} emitted undefined`);
  assert.ok(!output.includes("[object Object]"), `${language} stringified an object`);
  assert.ok(!output.includes("NaN"), `${language} emitted NaN`);

  let braces = 0;
  for (const character of output) {
    if (character === "{") braces++;
    if (character === "}") braces--;
    assert.ok(braces >= 0, `${language} emitted an unmatched closing brace`);
  }
  assert.equal(braces, 0, `${language} emitted unbalanced braces`);
}

test("seeded schema/query fuzzing produces deterministic output for every emitter", () => {
  const seed = readPositiveInteger("CODEGEN_FUZZ_SEED", DEFAULT_SEED);
  const runs = readPositiveInteger("CODEGEN_FUZZ_RUNS", DEFAULT_RUNS);
  const random = mulberry32(seed);

  for (let run = 0; run < runs; run++) {
    const suffix = `${seed.toString(16)}_${run}`;
    const leafType = `Leaf_${suffix}`;
    const branchType = `Branch_${suffix}`;
    const enumType = `Status_${suffix}`;
    const resultType = `Result_${suffix}`;
    const metadataType = `Metadata_${suffix}`;
    const scalarType = pick(random, ["String", "Int", "Float", "Boolean", "ID"]);
    const scalarFieldType = maybeList(random, scalarType);
    const leafFieldType = maybeList(random, leafType);
    const enumFieldType = maybeList(random, enumType);

    const schemaSource = `
      enum ${enumType} { ACTIVE_${run} INACTIVE_${run} }
      type ${leafType} { value_${run}: ${scalarFieldType} status_${run}: ${enumFieldType} }
      type ${branchType} { leaf_${run}: ${leafFieldType} count_${run}: Int! }
      input ${metadataType} { note_${run}: String flags_${run}: [Boolean!]! }
      input ${resultType} {
        accepted_${run}: Boolean!
        status_${run}: ${enumType}
        metadata_${run}: ${metadataType}
      }
      type Query {
        branch_${run}: ${branchType}
        branches_${run}: [${branchType}!]!
        direct_${run}: ${scalarFieldType}
      }
      type Mutation { target_${run}(result: ${resultType}!): String }
    `;

    const leafSelection = `value_${run} status_${run}`;
    const querySource = `query Fuzz_${run} {
      alias_${run}: branch_${run} { leaf_${run} { ${leafSelection} } count_${run} }
      branches_${run} { leaf_${run} { ${leafSelection} } count_${run} }
      direct_${run}
    }`;

    try {
      const schema = parseSchema(schemaSource);
      const query = parseQuery(querySource, schema);
      assert.equal(query.selections.length, 3);
      assert.equal(query.selections[0].name, `alias_${run}`);

      const targets = [{
        targetName: `target_${run}`,
        graphqlTargetName: `target_${run}`,
        selections: query.selections,
      }];
      const options = { enumsAsStr: random() < 0.5 ? [enumType] : [] };

      const zig = emitZig(schema, targets, options);
      const c = emitC(schema, targets, options);
      const goOptions = {
        ...options,
        modulePath: "github.com/Shopify/shopify-function-go",
        packageName: "generated",
      };
      const go = emitGo(schema, targets, goOptions);
      const ruby = emitRuby(schema, targets, options);

      assert.equal(zig, emitZig(schema, targets, options));
      assert.deepEqual(c, emitC(schema, targets, options));
      assert.equal(go, emitGo(schema, targets, goOptions));
      assert.deepEqual(ruby, emitRuby(schema, targets, options));
      assertSaneOutput("Zig", zig);
      assertSaneOutput("C header", c.header);
      assertSaneOutput("C source", c.source);
      assertSaneOutput("Go", go);
      assertSaneOutput("Ruby", ruby.ruby);
      assertSaneOutput("Ruby RBS", ruby.rbs);
      assertSaneOutput("Ruby LSP stubs", ruby.rbi);
    } catch (error) {
      assert.fail(`fuzz failure at seed=${seed}, run=${run}: ${String(error)}`);
    }
  }
});
