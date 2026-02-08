import { describe, it } from "node:test";
import * as assert from "node:assert";
import * as fs from "node:fs";
import * as path from "node:path";
import { parseSchema, parseQuery } from "../src/parser.js";
import { emitZig, camelToSnake } from "../src/emitters/zig.js";

// __dirname points to dist/tests/ after compilation, fixtures are at the project root
const fixturesDir = path.join(__dirname, "..", "..", "fixtures");

const schemaSource = fs.readFileSync(
  path.join(fixturesDir, "schema.graphql"),
  "utf-8"
);
const queryASource = fs.readFileSync(
  path.join(fixturesDir, "a.graphql"),
  "utf-8"
);
const queryBSource = fs.readFileSync(
  path.join(fixturesDir, "b.graphql"),
  "utf-8"
);

describe("parseSchema", () => {
  it("should extract query type fields", () => {
    const schema = parseSchema(schemaSource);
    assert.ok(schema.queryType);
    assert.strictEqual(schema.queryType.name, "Input");

    const fieldNames = schema.queryType.fields.map((f) => f.name);
    assert.ok(fieldNames.includes("id"));
    assert.ok(fieldNames.includes("num"));
    assert.ok(fieldNames.includes("name"));
    assert.ok(fieldNames.includes("targetAResult"));
    assert.ok(fieldNames.includes("country"));
    assert.ok(fieldNames.includes("optionalArray"));
  });

  it("should extract mutation targets", () => {
    const schema = parseSchema(schemaSource);
    assert.strictEqual(schema.mutationTargets.length, 2);
    assert.strictEqual(schema.mutationTargets[0].name, "targetA");
    assert.strictEqual(
      schema.mutationTargets[0].resultTypeName,
      "FunctionTargetAResult"
    );
    assert.strictEqual(schema.mutationTargets[1].name, "targetB");
    assert.strictEqual(
      schema.mutationTargets[1].resultTypeName,
      "FunctionTargetBResult"
    );
  });

  it("should extract input types", () => {
    const schema = parseSchema(schemaSource);
    assert.ok(schema.inputTypes.has("FunctionTargetAResult"));
    assert.ok(schema.inputTypes.has("FunctionTargetBResult"));
    assert.ok(schema.inputTypes.has("Operation"));
    assert.ok(schema.inputTypes.has("This"));
    assert.ok(schema.inputTypes.has("That"));
  });

  it("should detect @oneOf on Operation", () => {
    const schema = parseSchema(schemaSource);
    const operation = schema.inputTypes.get("Operation")!;
    assert.strictEqual(operation.isOneOf, true);
  });

  it("should extract enum types", () => {
    const schema = parseSchema(schemaSource);
    assert.ok(schema.enumTypes.has("CountryCode"));
    const cc = schema.enumTypes.get("CountryCode")!;
    assert.deepStrictEqual(cc.values, ["AC", "CA"]);
  });

  it("should detect @restrictTarget", () => {
    const schema = parseSchema(schemaSource);
    const targetAResult = schema.queryType!.fields.find(
      (f) => f.name === "targetAResult"
    )!;
    assert.deepStrictEqual(targetAResult.restrictTargets, ["test.target-b"]);
  });

  it("should extract custom scalars", () => {
    const schema = parseSchema(schemaSource);
    assert.ok(schema.customScalars.has("Date"));
    assert.ok(schema.customScalars.has("DateTime"));
    assert.ok(schema.customScalars.has("Decimal"));
  });
});

describe("parseQuery", () => {
  it("should parse query A selections", () => {
    const schema = parseSchema(schemaSource);
    const queryA = parseQuery(queryASource, schema, "test.target-a");
    const fieldNames = queryA.selections.map((s) => s.name);
    assert.ok(fieldNames.includes("id"));
    assert.ok(fieldNames.includes("num"));
    assert.ok(fieldNames.includes("name"));
    assert.ok(fieldNames.includes("date"));
    // targetAResult should NOT be in query A (it's restricted to target-b)
    assert.ok(!fieldNames.includes("targetAResult"));
  });

  it("should parse query B selections with @restrictTarget", () => {
    const schema = parseSchema(schemaSource);
    const queryB = parseQuery(queryBSource, schema, "test.target-b");
    const fieldNames = queryB.selections.map((s) => s.name);
    assert.ok(fieldNames.includes("id"));
    assert.ok(fieldNames.includes("targetAResult"));
    assert.ok(fieldNames.includes("optionalArray"));
  });
});

describe("Zig emitter", () => {
  it("should generate output structs with correct types", () => {
    const schema = parseSchema(schemaSource);
    const targets = [
      {
        targetName: "target_a",
        graphqlTargetName: "targetA",
        selections: parseQuery(queryASource, schema, "test.target-a")
          .selections,
      },
      {
        targetName: "target_b",
        graphqlTargetName: "targetB",
        selections: parseQuery(queryBSource, schema, "test.target-b")
          .selections,
      },
    ];

    const output = emitZig(schema, targets, { enumsAsStr: ["CountryCode"] });

    // Check output types
    assert.ok(output.includes("pub const FunctionTargetAResult = struct {"));
    assert.ok(output.includes("status: ?i32 = null,"));
    assert.ok(output.includes("pub const FunctionTargetBResult = struct {"));
    assert.ok(output.includes("name: ?[]const u8 = null,"));
    assert.ok(output.includes("operations: []const Operation,"));

    // Check @oneOf union
    assert.ok(output.includes("pub const Operation = union(enum) {"));
    assert.ok(output.includes("doThis: This,"));
    assert.ok(output.includes("doThat: That,"));

    // Check non-nullable struct fields
    assert.ok(output.includes("thisField: []const u8,"));
    assert.ok(output.includes("thatField: i32,"));
  });

  it("should generate per-target input modules", () => {
    const schema = parseSchema(schemaSource);
    const targets = [
      {
        targetName: "target_a",
        graphqlTargetName: "targetA",
        selections: parseQuery(queryASource, schema, "test.target-a")
          .selections,
      },
      {
        targetName: "target_b",
        graphqlTargetName: "targetB",
        selections: parseQuery(queryBSource, schema, "test.target-b")
          .selections,
      },
    ];

    const output = emitZig(schema, targets, { enumsAsStr: ["CountryCode"] });

    // Check target_a module
    assert.ok(output.includes("pub const target_a = struct {"));
    assert.ok(output.includes("pub fn id(self: Input) []const u8 {"));
    assert.ok(output.includes("pub fn num(self: Input) ?i32 {"));
    assert.ok(output.includes("pub fn name(self: Input) ?[]const u8 {"));

    // Check target_b module
    assert.ok(output.includes("pub const target_b = struct {"));
    assert.ok(output.includes("pub fn targetAResult(self: Input) ?i32 {"));

    // Check interned string usage
    assert.ok(output.includes('sf.wasm.internString("id")'));
    assert.ok(output.includes("getInternedObjProp(S.interned.?)"));
  });

  it("should use correct accessor bodies for different types", () => {
    const schema = parseSchema(schemaSource);
    const targets = [
      {
        targetName: "target_a",
        graphqlTargetName: "targetA",
        selections: parseQuery(queryASource, schema, "test.target-a")
          .selections,
      },
    ];

    const output = emitZig(schema, targets, { enumsAsStr: ["CountryCode"] });

    // String accessor should use readString
    assert.ok(output.includes("val.readString(str_buf)"));
    assert.ok(output.includes("val.stringLen()"));

    // Int accessor should use asNumber + @intFromFloat
    assert.ok(output.includes("val.asNumber()"));
    assert.ok(output.includes("@intFromFloat(n)"));
  });

  it("should generate array accessors for list fields", () => {
    const schema = parseSchema(schemaSource);
    const targets = [
      {
        targetName: "target_b",
        graphqlTargetName: "targetB",
        selections: parseQuery(queryBSource, schema, "test.target-b")
          .selections,
      },
    ];

    const output = emitZig(schema, targets, { enumsAsStr: ["CountryCode"] });

    assert.ok(output.includes("sf.ArrayAccessor([]const u8)"));
  });
});

describe("camelToSnake", () => {
  it("should convert camelCase to snake_case", () => {
    assert.strictEqual(camelToSnake("targetA"), "target_a");
    assert.strictEqual(camelToSnake("targetB"), "target_b");
    assert.strictEqual(camelToSnake("helloWorld"), "hello_world");
  });
});
