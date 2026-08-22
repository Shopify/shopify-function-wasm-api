import { describe, it } from "node:test";
import * as assert from "node:assert";
import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import {
  parseSchema,
  parseQuery,
  mergeJsonTypes,
  injectJsonOverrides,
} from "../src/parser.js";
import { emitZig, camelToSnake } from "../src/emitters/zig.js";
import { emitC } from "../src/emitters/c.js";
import { emitGo } from "../src/emitters/go.js";
import { emitRuby } from "../src/emitters/ruby.js";

// __dirname equivalent for ESM — points to dist/tests/ after compilation, fixtures are at the project root
const __dirname = path.dirname(fileURLToPath(import.meta.url));
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
const nestedSchemaSource = fs.readFileSync(
  path.join(fixturesDir, "nested_schema.graphql"),
  "utf-8"
);
const nestedQuerySource = fs.readFileSync(
  path.join(fixturesDir, "nested_query.graphql"),
  "utf-8"
);
const unionSchemaSource = fs.readFileSync(
  path.join(fixturesDir, "union_schema.graphql"),
  "utf-8"
);
const unionQuerySource = fs.readFileSync(
  path.join(fixturesDir, "union_query.graphql"),
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

    // Check direct property access
    assert.ok(output.includes('self.__value.getObjProp("id")'));
    assert.ok(output.includes('self.__value.getObjProp("num")'));
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

// --- Recursive nesting tests ---

function makeNestedTargets() {
  const schema = parseSchema(nestedSchemaSource);
  return {
    schema,
    targets: [
      {
        targetName: "cart_validation",
        graphqlTargetName: "cartValidation",
        selections: parseQuery(nestedQuerySource, schema).selections,
      },
    ],
  };
}

describe("parseQuery with nested types", () => {
  it("should parse nested object selections recursively", () => {
    const schema = parseSchema(nestedSchemaSource);
    const query = parseQuery(nestedQuerySource, schema);

    // Top level: cart, customer
    const topNames = query.selections.map((s) => s.name);
    assert.deepStrictEqual(topNames, ["cart", "customer"]);

    // cart -> lines, note
    const cart = query.selections.find((s) => s.name === "cart")!;
    const cartFieldNames = cart.selections.map((s) => s.name);
    assert.deepStrictEqual(cartFieldNames, ["lines", "note"]);

    // cart.lines -> quantity, merchandise, attributes
    const lines = cart.selections.find((s) => s.name === "lines")!;
    const lineFieldNames = lines.selections.map((s) => s.name);
    assert.deepStrictEqual(lineFieldNames, ["quantity", "merchandise", "attributes"]);

    // cart.lines.merchandise -> title, sku
    const merch = lines.selections.find((s) => s.name === "merchandise")!;
    assert.deepStrictEqual(
      merch.selections.map((s) => s.name),
      ["title", "sku"]
    );

    // cart.lines.attributes -> key, value
    const attrs = lines.selections.find((s) => s.name === "attributes")!;
    assert.deepStrictEqual(
      attrs.selections.map((s) => s.name),
      ["key", "value"]
    );

    // customer -> displayName, address
    const customer = query.selections.find((s) => s.name === "customer")!;
    assert.deepStrictEqual(
      customer.selections.map((s) => s.name),
      ["displayName", "address"]
    );

    // customer.address -> city, country
    const address = customer.selections.find((s) => s.name === "address")!;
    assert.deepStrictEqual(
      address.selections.map((s) => s.name),
      ["city", "country"]
    );
  });
});

describe("Zig emitter - nested types", () => {
  it("should generate nested object wrapper types", () => {
    const { schema, targets } = makeNestedTargets();
    const output = emitZig(schema, targets, { enumsAsStr: [] });

    // cart should return cart_T (nested object)
    assert.ok(output.includes("pub fn cart(self: Input) cart_T {"));
    assert.ok(output.includes("pub const cart_T = struct {"));

    // customer should return customer_T with nullable handling
    assert.ok(output.includes("pub fn customer(self: Input) ?customer_T {"));
    assert.ok(output.includes("pub const customer_T = struct {"));
  });

  it("should generate ArrayAccessor for list fields with sub-selections", () => {
    const { schema, targets } = makeNestedTargets();
    const output = emitZig(schema, targets, { enumsAsStr: [] });

    // lines should return ArrayAccessor(lines_Item)
    assert.ok(output.includes("pub fn lines(self: cart_T) sf.ArrayAccessor(lines_Item) {"));
    assert.ok(output.includes("pub const lines_Item = struct {"));
    assert.ok(output.includes("pub fn fromValue(val: sf.wasm.Value) lines_Item {"));
  });

  it("should generate accessors on nested item types", () => {
    const { schema, targets } = makeNestedTargets();
    const output = emitZig(schema, targets, { enumsAsStr: [] });

    // lines_Item should have quantity (scalar), merchandise (nested object), attributes (nested array)
    assert.ok(output.includes("pub fn quantity(self: lines_Item) i32 {"));
    assert.ok(output.includes("pub fn merchandise(self: lines_Item) merchandise_T {"));
    assert.ok(output.includes("pub const merchandise_T = struct {"));
    assert.ok(output.includes("pub fn title(self: merchandise_T) []const u8 {"));
    assert.ok(output.includes("pub fn sku(self: merchandise_T) ?[]const u8 {"));
  });

  it("should generate nested array of objects within arrays (3 levels deep)", () => {
    const { schema, targets } = makeNestedTargets();
    const output = emitZig(schema, targets, { enumsAsStr: [] });

    // attributes within lines_Item should be ArrayAccessor(attributes_Item)
    assert.ok(output.includes("pub fn attributes(self: lines_Item) sf.ArrayAccessor(attributes_Item) {"));
    assert.ok(output.includes("pub const attributes_Item = struct {"));
    assert.ok(output.includes("pub fn key(self: attributes_Item) []const u8 {"));
    assert.ok(output.includes("pub fn value(self: attributes_Item) ?[]const u8 {"));
  });

  it("should generate nested object within nullable parent", () => {
    const { schema, targets } = makeNestedTargets();
    const output = emitZig(schema, targets, { enumsAsStr: [] });

    // address inside customer_T
    assert.ok(output.includes("pub fn address(self: customer_T) ?address_T {"));
    assert.ok(output.includes("pub const address_T = struct {"));
    assert.ok(output.includes("pub fn city(self: address_T) []const u8 {"));
    assert.ok(output.includes("pub fn country(self: address_T) []const u8 {"));
  });
});

describe("C emitter - nested types", () => {
  it("should generate nested wrapper structs and accessor functions", () => {
    const { schema, targets } = makeNestedTargets();
    const { header, source } = emitC(schema, targets, { enumsAsStr: [] });

    // Should have wrapper types for nested objects
    assert.ok(header.includes("typedef struct cart_validation_cart {"));
    assert.ok(header.includes("typedef struct cart_validation_cart_lines_Item {"));

    // Should have accessor to get cart (returns wrapper)
    assert.ok(header.includes("cart_validation_cart cart_validation_get_cart(cart_validation_Input input)"));

    // Should have array accessor for lines
    assert.ok(header.includes("cart_validation_cart_lines_len"));
    assert.ok(header.includes("cart_validation_cart_lines_get"));

    // Should have scalar accessor on item
    assert.ok(header.includes("int32_t cart_validation_cart_lines_Item_get_quantity(cart_validation_cart_lines_Item input)"));

    // Implementation should have direct property lookups
    assert.ok(source.includes('shopify_function_input_get_obj_prop(input.__value, (const uint8_t*)"cart"'));
    assert.ok(source.includes('shopify_function_input_get_obj_prop(input.__value, (const uint8_t*)"quantity"'));
  });

  it("should generate nested object accessors within array items", () => {
    const { schema, targets } = makeNestedTargets();
    const { header } = emitC(schema, targets, { enumsAsStr: [] });

    // merchandise inside lines_Item should be a nested wrapper
    assert.ok(header.includes("cart_validation_cart_lines_Item_get_merchandise"));
  });
});

describe("Go emitter - nested types", () => {
  it("should generate nested wrapper structs and methods", () => {
    const { schema, targets } = makeNestedTargets();
    const output = emitGo(schema, targets, {
      enumsAsStr: [],
      modulePath: "github.com/Shopify/shopify-function-go",
      packageName: "generated",
    });

    // Root input type
    assert.ok(output.includes("type Cart_validationInput struct {"));

    // Nested cart type with accessor
    assert.ok(output.includes("type Cart_validationInputCart struct {"));
    assert.ok(output.includes("func (input Cart_validationInput) Cart() Cart_validationInputCart {"));

    // Array types for lines
    assert.ok(output.includes("type Cart_validationInputCartLinesArray struct {"));
    assert.ok(output.includes("type Cart_validationInputCartLinesItem struct {"));
    assert.ok(output.includes("func (arr Cart_validationInputCartLinesArray) Len() uint32 {"));
    assert.ok(output.includes("func (arr Cart_validationInputCartLinesArray) Get(index uint32) Cart_validationInputCartLinesItem {"));

    // Scalar accessor on item
    assert.ok(output.includes("func (input Cart_validationInputCartLinesItem) Quantity() int32 {"));
  });

  it("should generate nested object within array item", () => {
    const { schema, targets } = makeNestedTargets();
    const output = emitGo(schema, targets, {
      enumsAsStr: [],
      modulePath: "github.com/Shopify/shopify-function-go",
      packageName: "generated",
    });

    // merchandise inside lines item
    assert.ok(output.includes("type Cart_validationInputCartLinesItemMerchandise struct {"));
    assert.ok(output.includes("func (input Cart_validationInputCartLinesItem) Merchandise() Cart_validationInputCartLinesItemMerchandise {"));
    assert.ok(output.includes("func (input Cart_validationInputCartLinesItemMerchandise) Title() string {"));
  });

  it("should generate nullable nested object accessor", () => {
    const { schema, targets } = makeNestedTargets();
    const output = emitGo(schema, targets, {
      enumsAsStr: [],
      modulePath: "github.com/Shopify/shopify-function-go",
      packageName: "generated",
    });

    // customer is nullable
    assert.ok(output.includes("func (input Cart_validationInput) Customer() (*Cart_validationInputCustomer, bool) {"));
    // address inside customer is also nullable
    assert.ok(output.includes("func (input Cart_validationInputCustomer) Address() (*Cart_validationInputCustomerAddress, bool) {"));
  });
});

// --- Union type tests ---

function makeUnionTargets() {
  const schema = parseSchema(unionSchemaSource);
  return {
    schema,
    targets: [
      {
        targetName: "cart_validation",
        graphqlTargetName: "cartValidation",
        selections: parseQuery(unionQuerySource, schema).selections,
      },
    ],
  };
}

describe("parseSchema with union types", () => {
  it("should extract union types", () => {
    const schema = parseSchema(unionSchemaSource);
    assert.ok(schema.unionTypes.has("Merchandise"));
    const merch = schema.unionTypes.get("Merchandise")!;
    assert.deepStrictEqual(merch.memberTypes, ["ProductVariant", "CustomProduct"]);
  });
});

describe("parseQuery with inline fragments", () => {
  it("should parse inline fragments for union types", () => {
    const schema = parseSchema(unionSchemaSource);
    const query = parseQuery(unionQuerySource, schema);

    // Navigate to merchandise field
    const cart = query.selections.find((s) => s.name === "cart")!;
    const lines = cart.selections.find((s) => s.name === "lines")!;
    const merchandise = lines.selections.find((s) => s.name === "merchandise")!;

    // merchandise should have inline fragments, not regular selections
    assert.strictEqual(merchandise.selections.length, 0);
    assert.ok(merchandise.inlineFragments);
    assert.strictEqual(merchandise.inlineFragments.length, 2);

    // ProductVariant fragment
    const pvFragment = merchandise.inlineFragments.find((f) => f.typeName === "ProductVariant")!;
    assert.ok(pvFragment);
    const pvFields = pvFragment.selections.map((s) => s.name);
    assert.deepStrictEqual(pvFields, ["id", "product"]);

    // product has sub-selection id
    const product = pvFragment.selections.find((s) => s.name === "product")!;
    assert.deepStrictEqual(product.selections.map((s) => s.name), ["id"]);

    // CustomProduct fragment
    const cpFragment = merchandise.inlineFragments.find((f) => f.typeName === "CustomProduct")!;
    assert.ok(cpFragment);
    assert.deepStrictEqual(cpFragment.selections.map((s) => s.name), ["title"]);
  });

  it("should skip __typename fields", () => {
    const schema = parseSchema(unionSchemaSource);
    const query = parseQuery(unionQuerySource, schema);

    const cart = query.selections.find((s) => s.name === "cart")!;
    const lines = cart.selections.find((s) => s.name === "lines")!;
    const merchandise = lines.selections.find((s) => s.name === "merchandise")!;

    // __typename should not appear as a regular field selection
    assert.strictEqual(merchandise.selections.length, 0);
    // And not in inline fragment selections either
    for (const frag of merchandise.inlineFragments!) {
      assert.ok(!frag.selections.some((s) => s.name === "__typename"));
    }
  });

  it("should handle aliases", () => {
    const schema = parseSchema(unionSchemaSource);
    const aliasQuery = `query Input {
      cart {
        lines {
          itemId: id
          qty: quantity
        }
      }
    }`;
    const query = parseQuery(aliasQuery, schema);

    const cart = query.selections.find((s) => s.name === "cart")!;
    const lines = cart.selections.find((s) => s.name === "lines")!;
    const fieldNames = lines.selections.map((s) => s.name);
    assert.deepStrictEqual(fieldNames, ["itemId", "qty"]);
  });
});

describe("Zig emitter - union types", () => {
  it("should generate union(enum) with fromValue for union fields", () => {
    const { schema, targets } = makeUnionTargets();
    const output = emitZig(schema, targets, { enumsAsStr: [] });

    // Should generate merchandise_T as union(enum)
    assert.ok(output.includes("pub const merchandise_T = union(enum) {"));
    assert.ok(output.includes("ProductVariant: merchandise_ProductVariant,"));
    assert.ok(output.includes("CustomProduct: merchandise_CustomProduct,"));
    assert.ok(output.includes("Other,"));

    // Should have fromValue that reads __typename
    assert.ok(output.includes("pub fn fromValue(val: sf.wasm.Value) merchandise_T {"));
    assert.ok(output.includes('val.getObjProp("__typename")'));
    assert.ok(output.includes('sf.wasm.strEql(buf, "ProductVariant")'));
    assert.ok(output.includes('sf.wasm.strEql(buf, "CustomProduct")'));
  });

  it("should generate per-variant structs with accessors", () => {
    const { schema, targets } = makeUnionTargets();
    const output = emitZig(schema, targets, { enumsAsStr: [] });

    // ProductVariant struct
    assert.ok(output.includes("pub const merchandise_ProductVariant = struct {"));
    assert.ok(output.includes("pub fn id(self: merchandise_ProductVariant)"));
    assert.ok(output.includes("pub fn product(self: merchandise_ProductVariant)"));

    // CustomProduct struct
    assert.ok(output.includes("pub const merchandise_CustomProduct = struct {"));
    assert.ok(output.includes("pub fn title(self: merchandise_CustomProduct)"));
  });

  it("should generate accessor that calls fromValue", () => {
    const { schema, targets } = makeUnionTargets();
    const output = emitZig(schema, targets, { enumsAsStr: [] });

    // The merchandise accessor should call merchandise_T.fromValue
    assert.ok(output.includes("pub fn merchandise(self: lines_Item) merchandise_T {"));
    assert.ok(output.includes("return merchandise_T.fromValue(val);"));
  });
});

describe("C emitter - union types", () => {
  it("should generate tag enum and tagged wrapper for union fields", () => {
    const { schema, targets } = makeUnionTargets();
    const { header, source } = emitC(schema, targets, { enumsAsStr: [] });

    // Tag enum
    assert.ok(header.includes("typedef enum cart_validation_cart_lines_Item_merchandise_Tag {"));
    assert.ok(header.includes("cart_validation_cart_lines_Item_merchandise_Tag_ProductVariant,"));
    assert.ok(header.includes("cart_validation_cart_lines_Item_merchandise_Tag_CustomProduct,"));
    assert.ok(header.includes("cart_validation_cart_lines_Item_merchandise_Tag_Other,"));

    // Tagged wrapper struct
    assert.ok(header.includes("typedef struct cart_validation_cart_lines_Item_merchandise {"));
    assert.ok(header.includes("cart_validation_cart_lines_Item_merchandise_Tag tag;"));
  });

  it("should generate cast function declarations", () => {
    const { schema, targets } = makeUnionTargets();
    const { header } = emitC(schema, targets, { enumsAsStr: [] });

    assert.ok(header.includes("cart_validation_cart_lines_Item_merchandise_ProductVariant cart_validation_cart_lines_Item_merchandise_as_ProductVariant"));
    assert.ok(header.includes("cart_validation_cart_lines_Item_merchandise_CustomProduct cart_validation_cart_lines_Item_merchandise_as_CustomProduct"));
  });

  it("should generate __typename dispatch in accessor implementation", () => {
    const { schema, targets } = makeUnionTargets();
    const { source } = emitC(schema, targets, { enumsAsStr: [] });

    // Should read __typename
    assert.ok(source.includes('"__typename"'));
    assert.ok(source.includes('sf_str_eq((const char*)tn_buf, "ProductVariant"'));
    assert.ok(source.includes('sf_str_eq((const char*)tn_buf, "CustomProduct"'));
  });
});

describe("Go emitter - union types", () => {
  it("should generate interface and variant structs for union fields", () => {
    const { schema, targets } = makeUnionTargets();
    const output = emitGo(schema, targets, {
      enumsAsStr: [],
      modulePath: "github.com/Shopify/shopify-function-go",
      packageName: "generated",
    });

    // Interface
    assert.ok(output.includes("type Cart_validationInputCartLinesItemMerchandise interface {"));
    assert.ok(output.includes("isCart_validationInputCartLinesItemMerchandise()"));

    // Other variant
    assert.ok(output.includes("type Cart_validationInputCartLinesItemMerchandiseOther struct{}"));

    // ProductVariant variant
    assert.ok(output.includes("type Cart_validationInputCartLinesItemMerchandiseProductVariant struct {"));

    // CustomProduct variant
    assert.ok(output.includes("type Cart_validationInputCartLinesItemMerchandiseCustomProduct struct {"));
  });

  it("should generate __typename switch in accessor", () => {
    const { schema, targets } = makeUnionTargets();
    const output = emitGo(schema, targets, {
      enumsAsStr: [],
      modulePath: "github.com/Shopify/shopify-function-go",
      packageName: "generated",
    });

    // Accessor should read __typename and switch
    assert.ok(output.includes('typename := val.GetObjProp("__typename").ReadStringAlloc()'));
    assert.ok(output.includes('case "ProductVariant"'));
    assert.ok(output.includes('case "CustomProduct"'));
  });

  it("should generate per-variant field accessors", () => {
    const { schema, targets } = makeUnionTargets();
    const output = emitGo(schema, targets, {
      enumsAsStr: [],
      modulePath: "github.com/Shopify/shopify-function-go",
      packageName: "generated",
    });

    // ProductVariant should have Id and Product accessors
    assert.ok(output.includes("func (input Cart_validationInputCartLinesItemMerchandiseProductVariant) Id()"));
    assert.ok(output.includes("func (input Cart_validationInputCartLinesItemMerchandiseProductVariant) Product()"));

    // CustomProduct should have Title accessor
    assert.ok(output.includes("func (input Cart_validationInputCartLinesItemMerchandiseCustomProduct) Title()"));
  });
});

// --- JSON override tests ---

const jsonSchemaSource = fs.readFileSync(
  path.join(fixturesDir, "json_schema.graphql"),
  "utf-8"
);
const jsonQuerySource = fs.readFileSync(
  path.join(fixturesDir, "json_query.graphql"),
  "utf-8"
);
const jsonTypesSource = fs.readFileSync(
  path.join(fixturesDir, "json_types.graphql"),
  "utf-8"
);

function makeJsonTargets(overrideKey: string = "jsonValue") {
  const schema = parseSchema(jsonSchemaSource);
  mergeJsonTypes(jsonTypesSource, schema);
  const query = parseQuery(jsonQuerySource, schema);
  const overrides = new Map([[overrideKey, "Configuration"]]);
  injectJsonOverrides(query.selections, overrides, schema);
  return {
    schema,
    query,
    targets: [
      {
        targetName: "handle_target",
        graphqlTargetName: "handleTarget",
        selections: query.selections,
      },
    ],
  };
}

describe("mergeJsonTypes", () => {
  it("should merge object types from json-types file into schema model", () => {
    const schema = parseSchema(jsonSchemaSource);
    mergeJsonTypes(jsonTypesSource, schema);
    assert.ok(schema.objectTypes.has("Configuration"));
    assert.ok(schema.objectTypes.has("ConfigItem"));

    const config = schema.objectTypes.get("Configuration")!;
    const fieldNames = config.fields.map((f) => f.name);
    assert.deepStrictEqual(fieldNames, ["limit", "message", "items"]);
  });
});

describe("injectJsonOverrides", () => {
  it("should replace JSON scalar field with typed sub-selections", () => {
    const { query } = makeJsonTargets();
    const metafield = query.selections.find((s) => s.name === "metafield")!;
    const jsonValue = metafield.selections.find((s) => s.name === "jsonValue")!;

    // jsonValue should now have sub-selections from Configuration type
    assert.ok(jsonValue.selections.length > 0);
    const subFieldNames = jsonValue.selections.map((s) => s.name);
    assert.deepStrictEqual(subFieldNames, ["limit", "message", "items"]);
  });

  it("should handle nested override types", () => {
    const { query } = makeJsonTargets();
    const metafield = query.selections.find((s) => s.name === "metafield")!;
    const jsonValue = metafield.selections.find((s) => s.name === "jsonValue")!;

    // items field should have sub-selections from ConfigItem type
    const items = jsonValue.selections.find((s) => s.name === "items")!;
    assert.ok(items.selections.length > 0);
    const itemFieldNames = items.selections.map((s) => s.name);
    assert.deepStrictEqual(itemFieldNames, ["name", "value"]);
  });

  it("should match by simple field name", () => {
    const { query } = makeJsonTargets("jsonValue");
    const metafield = query.selections.find((s) => s.name === "metafield")!;
    const jsonValue = metafield.selections.find((s) => s.name === "jsonValue")!;
    assert.ok(jsonValue.selections.length > 0);
  });

  it("should match by parent.field path", () => {
    const { query } = makeJsonTargets("metafield.jsonValue");
    const metafield = query.selections.find((s) => s.name === "metafield")!;
    const jsonValue = metafield.selections.find((s) => s.name === "jsonValue")!;
    assert.ok(jsonValue.selections.length > 0);
  });

  it("should not match when path suffix does not match", () => {
    const schema = parseSchema(jsonSchemaSource);
    mergeJsonTypes(jsonTypesSource, schema);
    const query = parseQuery(jsonQuerySource, schema);
    const overrides = new Map([["nonExistentField", "Configuration"]]);
    injectJsonOverrides(query.selections, overrides, schema);

    const metafield = query.selections.find((s) => s.name === "metafield")!;
    const jsonValue = metafield.selections.find((s) => s.name === "jsonValue")!;
    assert.strictEqual(jsonValue.selections.length, 0);
  });

  it("should not inject if field already has sub-selections", () => {
    const schema = parseSchema(jsonSchemaSource);
    mergeJsonTypes(jsonTypesSource, schema);
    const query = parseQuery(jsonQuerySource, schema);

    // metafield already has sub-selections (jsonValue, type, value)
    const overrides = new Map([["metafield", "Configuration"]]);
    injectJsonOverrides(query.selections, overrides, schema);

    // metafield should keep its original sub-selections, not be replaced
    const metafield = query.selections.find((s) => s.name === "metafield")!;
    const fieldNames = metafield.selections.map((s) => s.name);
    assert.deepStrictEqual(fieldNames, ["jsonValue", "type", "value"]);
  });
});

describe("Zig emitter - JSON override", () => {
  it("should generate nested struct type for overridden JSON field", () => {
    const { schema, targets } = makeJsonTargets();
    const output = emitZig(schema, targets, { enumsAsStr: [] });

    // jsonValue should generate a nested struct type
    assert.ok(output.includes("pub const jsonValue_T = struct {"));
    assert.ok(output.includes("pub fn limit(self: jsonValue_T) i32 {"));
    assert.ok(output.includes("pub fn message(self: jsonValue_T) []const u8 {"));

    // items should be an array accessor with a nested item type
    assert.ok(
      output.includes("pub fn items(self: jsonValue_T) sf.ArrayAccessor(items_Item) {")
    );
    assert.ok(output.includes("pub const items_Item = struct {"));
    assert.ok(output.includes("pub fn name(self: items_Item) []const u8 {"));
  });
});

describe("C emitter - JSON override", () => {
  it("should generate nested wrapper structs for overridden JSON field", () => {
    const { schema, targets } = makeJsonTargets();
    const { header } = emitC(schema, targets, { enumsAsStr: [] });

    // Should have wrapper type for jsonValue
    assert.ok(header.includes("handle_target_metafield_jsonValue"));

    // Should have accessor for limit
    assert.ok(
      header.includes("handle_target_metafield_jsonValue_get_limit") ||
        header.includes("handle_target_metafield_jsonValue_Item_get_limit")
    );
  });
});

describe("Go emitter - JSON override", () => {
  it("should generate nested wrapper struct for overridden JSON field", () => {
    const { schema, targets } = makeJsonTargets();
    const output = emitGo(schema, targets, {
      enumsAsStr: [],
      modulePath: "github.com/Shopify/shopify-function-go",
      packageName: "generated",
    });

    // Should have nested type for jsonValue
    assert.ok(output.includes("Handle_targetInputMetafieldJsonValue"));

    // Should have accessor for limit
    assert.ok(output.includes("Limit()"));
    assert.ok(output.includes("Message()"));
  });
});

function makeRubyTargets(queryName: "a" | "b" = "a") {
  const schema = parseSchema(schemaSource);
  const source = queryName === "a" ? queryASource : queryBSource;
  return {
    schema,
    targets: [
      {
        targetName: `target_${queryName}`,
        graphqlTargetName: `target${queryName.toUpperCase()}`,
        selections: parseQuery(source, schema, `test.target-${queryName}`)
          .selections,
      },
    ],
  };
}

describe("Ruby emitter", () => {
  it("should emit a header, a private marker, and snake_case accessors", () => {
    const { schema, targets } = makeRubyTargets();
    const { ruby } = emitRuby(schema, targets, { enumsAsStr: ["CountryCode"] });

    assert.ok(ruby.startsWith("# frozen_string_literal: true\n"));
    assert.ok(ruby.includes("# Generated by shopify-function-codegen --language ruby."));
    assert.ok(ruby.includes("\nprivate\n"));

    // Accessors are top-level one-parameter methods so rubywat can lower
    // `input.date_time` into `date_time(input)`.
    assert.ok(
      ruby.includes(
        '#: (_TargetAInput) -> String?\ndef date_time(target_a_input)\n  target_a_input["dateTime"]\nend'
      )
    );
  });

  it("should map GraphQL scalars onto RBS types", () => {
    const { schema, targets } = makeRubyTargets("b");
    const { ruby } = emitRuby(schema, targets, { enumsAsStr: ["CountryCode"] });

    assert.ok(ruby.includes("#: (_TargetBInput) -> String\ndef id("));
    assert.ok(ruby.includes("#: (_TargetBInput) -> Integer?\ndef target_a_result("));
    assert.ok(ruby.includes("#: (_TargetBInput) -> Array[String]?\ndef optional_array("));
    assert.ok(
      ruby.includes(
        "#: (_TargetBInput) -> Array[Array[String]?]?\ndef optional_array_of_optional_arrays("
      )
    );
  });

  it("should emit output type helpers returning hash literals", () => {
    const { schema, targets } = makeRubyTargets();
    const { ruby } = emitRuby(schema, targets, { enumsAsStr: ["CountryCode"] });

    assert.ok(ruby.includes("# Output type helpers used by rubywat lowering."));
    assert.ok(
      ruby.includes(
        "#: (Integer?) -> FunctionTargetAResult\ndef __FunctionTargetAResult_new(status)\n  {\n    status: status\n  }\nend"
      )
    );
  });

  it("should emit RBS interfaces plus an Object method per accessor", () => {
    const { schema, targets } = makeRubyTargets();
    const { rbs } = emitRuby(schema, targets, { enumsAsStr: ["CountryCode"] });

    assert.ok(rbs.includes("interface _TargetAInput\n"));
    assert.ok(rbs.includes("  def id: () -> String\n"));
    assert.ok(rbs.includes("  def num: () -> Integer?\n"));

    // The Object block is what makes the lowered `id(input)` form type-check.
    assert.ok(rbs.includes("class Object\n"));
    assert.ok(rbs.includes("  def id: (_TargetAInput) -> String\n"));

    // Output objects are classes with an all-keyword constructor.
    assert.ok(
      rbs.includes(
        "  def self.new: (status: Integer?) -> FunctionTargetAResult\n"
      )
    );
  });

  it("should emit LSP stub classes with placeholder return values", () => {
    const { schema, targets } = makeRubyTargets();
    const { rbi } = emitRuby(schema, targets, { enumsAsStr: ["CountryCode"] });

    assert.ok(rbi.includes("# Ruby LSP stubs only. Do not compile this file with rubywat."));
    assert.ok(rbi.includes("class TargetAInput\n"));
    assert.ok(rbi.includes("  def id\n    \"\"\n  end\n"));
    assert.ok(rbi.includes("  def num\n    0\n  end\n"));
    assert.ok(rbi.includes("  def self.new(status:)\n    {}\n  end\n"));
  });

  it("should emit accessors and output helpers for every target", () => {
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

    const { ruby, rbs } = emitRuby(schema, targets, {
      enumsAsStr: ["CountryCode"],
    });

    assert.ok(ruby.includes("def __FunctionTargetAResult_new(status)"));
    assert.ok(ruby.includes("def __FunctionTargetBResult_new(name, operations)"));
    assert.ok(rbs.includes("interface _TargetAInput\n"));
    assert.ok(rbs.includes("interface _TargetBInput\n"));

    // `id` is selected by both queries, so it takes a union of receivers. Both
    // return String, so the return type collapses instead of becoming a union.
    assert.ok(ruby.includes("#: (_TargetAInput | _TargetBInput) -> String\n"));
    assert.ok(ruby.includes('def id(receiver)\n  receiver["id"]\nend'));
  });

  it("should wrap prose doc comments at 90 columns", () => {
    const { schema, targets } = makeNestedTargets();
    const result = emitRuby(schema, targets, { enumsAsStr: [] });

    for (const file of [result.ruby, result.rbs, result.rbi]) {
      for (const line of file.split("\n")) {
        // `#:` lines are RBS annotations, not prose — they must stay on one line.
        if (!line.trimStart().startsWith("# ")) continue;
        assert.ok(line.length <= 90, `comment line exceeds 90 columns: ${line}`);
      }
    }

    // The nested fixture has type names long enough to force a greedy wrap,
    // with the indent and the "# " prefix counted against the 90 columns.
    assert.ok(
      result.rbs.includes(
        "  # Reads the lines field from CartValidationInputCart. Returns\n  # Array[_CartValidationInputCartLinesItem].\n"
      )
    );
  });
});

describe("Ruby emitter - nested types", () => {
  it("should declare one interface per nested selection", () => {
    const { schema, targets } = makeNestedTargets();
    const { rbs } = emitRuby(schema, targets, { enumsAsStr: [] });

    assert.ok(rbs.includes("interface _CartValidationInput\n"));
    assert.ok(rbs.includes("interface _CartValidationInputCart\n"));
    assert.ok(rbs.includes("interface _CartValidationInputCartLinesItem\n"));
    assert.ok(
      rbs.includes("interface _CartValidationInputCartLinesItemMerchandise\n")
    );
    assert.ok(rbs.includes("interface _CartValidationInputCustomerAddress\n"));
  });

  it("should wrap list selections in Array and suffix item types", () => {
    const { schema, targets } = makeNestedTargets();
    const { ruby } = emitRuby(schema, targets, { enumsAsStr: [] });

    assert.ok(
      ruby.includes(
        "#: (_CartValidationInputCart) -> Array[_CartValidationInputCartLinesItem]"
      )
    );
    assert.ok(
      ruby.includes(
        '#: (_CartValidationInputCartLinesItem) -> Integer\ndef quantity(cart_validation_input_cart_lines_item)\n  cart_validation_input_cart_lines_item["quantity"]\nend'
      )
    );
  });

  it("should mark nullable object selections with a trailing question mark", () => {
    const { schema, targets } = makeNestedTargets();
    const { ruby } = emitRuby(schema, targets, { enumsAsStr: [] });

    assert.ok(
      ruby.includes(
        "#: (_CartValidationInput) -> _CartValidationInputCustomer?\ndef customer("
      )
    );
  });
});

describe("Ruby emitter - union types", () => {
  it("should merge inline fragment fields into one receiver surface", () => {
    const { schema, targets } = makeUnionTargets();
    const { rbs } = emitRuby(schema, targets, { enumsAsStr: [] });

    // ProductVariant.product and CustomProduct.title both land on the union receiver.
    const merchandise = rbs.slice(
      rbs.indexOf("interface _CartValidationInputCartLinesItemMerchandise\n")
    );
    assert.ok(merchandise.includes("  def product: () -> "));
    assert.ok(merchandise.includes("  def title: () -> String?\n"));
  });

  it("should make variant-specific fields nullable", () => {
    const { schema, targets } = makeUnionTargets();
    const { ruby } = emitRuby(schema, targets, { enumsAsStr: [] });

    // title is non-null on CustomProduct but absent from ProductVariant.
    assert.ok(
      ruby.includes(
        "#: (_CartValidationInputCartLinesItemMerchandise) -> String?\ndef title("
      )
    );
    assert.ok(
      ruby.includes(
        "#: (_CartValidationInputCartLinesItemMerchandise) -> _CartValidationInputCartLinesItemMerchandiseProduct?\ndef product("
      )
    );
  });

  it("should keep the leading underscores of __typename", () => {
    const { schema, targets } = makeUnionTargets();
    const { ruby, rbs } = emitRuby(schema, targets, { enumsAsStr: [] });

    assert.ok(
      ruby.includes(
        '#: (_CartValidationInputCartLinesItemMerchandise) -> String\ndef __typename(cart_validation_input_cart_lines_item_merchandise)\n  cart_validation_input_cart_lines_item_merchandise["__typename"]\nend'
      )
    );
    assert.ok(rbs.includes("  def __typename: () -> String\n"));
  });

  it("should emit positionally correlated receiver and return unions", () => {
    const { schema, targets } = makeUnionTargets();
    const { ruby } = emitRuby(schema, targets, { enumsAsStr: [] });

    // `id` is selected on three different receivers. Both unions stay positional
    // (String appears twice) so rubywat can correlate receiver N with return N.
    assert.ok(
      ruby.includes(
        "#: (_CartValidationInputCartLinesItem | _CartValidationInputCartLinesItemMerchandise | _CartValidationInputCartLinesItemMerchandiseProduct) -> (String | String? | String)"
      )
    );
    assert.ok(ruby.includes('def id(receiver)\n  receiver["id"]\nend'));
  });
});

describe("Ruby emitter - oneOf outputs", () => {
  it("should emit one variant constructor per @oneOf field", () => {
    const { schema, targets } = makeRubyTargets("b");
    const { ruby, rbs } = emitRuby(schema, targets, {
      enumsAsStr: ["CountryCode"],
    });

    assert.ok(!ruby.includes("def __Operation_new("));
    assert.ok(ruby.includes("#: (String) -> Operation\ndef __Operation_do_this(this_field)"));
    assert.ok(ruby.includes("#: (Integer) -> Operation\ndef __Operation_do_that(that_field)"));
    assert.ok(rbs.includes("  def self.do_this: (this_field: String) -> Operation\n"));
    assert.ok(rbs.includes("  def self.do_that: (that_field: Integer) -> Operation\n"));
  });

  it("should inline the variant payload fields under the variant key", () => {
    const { schema, targets } = makeRubyTargets("b");
    const { ruby } = emitRuby(schema, targets, {
      enumsAsStr: ["CountryCode"],
    });

    assert.ok(
      ruby.includes(
        "def __Operation_do_this(this_field)\n  {\n    doThis: {\n      thisField: this_field\n    }\n  }\nend"
      )
    );
  });
});

describe("Ruby emitter - JSON override", () => {
  it("should generate typed accessors for the overridden JSON field", () => {
    const { schema, targets } = makeJsonTargets();
    const { ruby, rbs } = emitRuby(schema, targets, { enumsAsStr: [] });

    assert.ok(
      ruby.includes(
        "#: (_HandleTargetInputMetafield) -> _HandleTargetInputMetafieldJsonValue\ndef json_value("
      )
    );
    assert.ok(
      ruby.includes("#: (_HandleTargetInputMetafieldJsonValue) -> Integer\ndef limit(")
    );
    assert.ok(
      ruby.includes(
        "#: (_HandleTargetInputMetafieldJsonValue) -> Array[_HandleTargetInputMetafieldJsonValueItemsItem]\ndef items("
      )
    );
    assert.ok(rbs.includes("interface _HandleTargetInputMetafieldJsonValueItemsItem\n"));
  });

  it("should fall back to the schema mutation target for output helpers", () => {
    const { schema, targets } = makeJsonTargets();
    const { ruby } = emitRuby(schema, targets, { enumsAsStr: [] });

    // The target has no resultTypeName, so it is resolved from the schema.
    assert.ok(ruby.includes("def __FunctionTargetResult_new(status)"));
  });
});
