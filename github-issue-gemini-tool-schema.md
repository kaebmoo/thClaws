## Summary

When using the native Gemini provider with tools enabled, thClaws can send tool schemas that Gemini rejects before the model runs. The request fails with HTTP 400 from `generativelanguage.googleapis.com` because `functionDeclarations[].parameters` contains JSON Schema fields and numeric enum values that Gemini's `Schema` type does not accept.

## Error

Example error from a GUI run with Gemini:

```text
HTTP error: Invalid value at 'tools[0].function_declarations[7].parameters.properties[1].value.enum[0]' (TYPE_STRING), 0
Invalid value at 'tools[0].function_declarations[7].parameters.properties[1].value.enum[1]' (TYPE_STRING), 1
Invalid value at 'tools[0].function_declarations[7].parameters.properties[1].value.enum[2]' (TYPE_STRING), 2
Invalid JSON payload received. Unknown name "$schema" at 'tools[0].function_declarations[43].parameters': Cannot find field.
Invalid JSON payload received. Unknown name "additionalProperties" at 'tools[0].function_declarations[43].parameters': Cannot find field.
Invalid JSON payload received. Unknown name "propertyNames" at 'tools[0].function_declarations[47].parameters.properties[0].value': Cannot find field.
```

The full error repeats for multiple tool declarations.

## Likely root cause

`GeminiProvider::build_body` passes `t.input_schema` directly into Gemini `functionDeclarations[].parameters`:

```rust
"parameters": t.input_schema,
```

Gemini's `parameters` field is a `Schema` object, not arbitrary JSON Schema. The Gemini API docs describe it as a select OpenAPI subset. In that `Schema`, `enum[]` is documented as `string[]` for string enum values, and fields like `$schema`, `additionalProperties`, and `propertyNames` are not accepted under `parameters`.

Relevant docs:

- https://ai.google.dev/gemini-api/docs/function-calling
- https://ai.google.dev/api/caching#Schema

Concrete schemas in the repo that trigger this include numeric integer enums:

- `crates/core/src/tools/pdf_create.rs`: `outline_depth` has `"enum": [0, 1, 2]`
- `crates/core/src/tools/epub_create.rs`: `chapter_split` has `"enum": [0, 1, 2]`

Additional JSON Schema keywords can also come from MCP tool schemas.

## Reproduction

1. Configure the native Gemini provider.
2. Run the GUI or CLI with tools enabled.
3. Send a prompt that starts a normal agent turn.
4. Gemini returns HTTP 400 while validating `tools[0].function_declarations`.

This happens before any model response, so it is request schema validation rather than model behavior.

## Expected behavior

Gemini requests should only include tool declaration schemas compatible with Gemini's `Schema` type, or use a compatible alternate field. Unsupported JSON Schema keywords and non-string enum values should not be sent in `functionDeclarations[].parameters`.

## Local validation

I tested a local fix that sanitizes tool schemas before sending them to Gemini:

- removes unsupported JSON Schema fields such as `$schema`, `additionalProperties`, and `propertyNames`
- recursively sanitizes nested `properties`, `items`, and `anyOf`
- keeps string enums
- drops non-string enums under `parameters`

Validation performed:

```sh
cargo test -p thclaws-core providers::gemini::tests::build_body_sanitizes_tool_schema_for_gemini
cargo test -p thclaws-core providers::gemini::tests
cargo build --features gui --bin thclaws
```

Result:

- the new regression test fails before the sanitizer because `$schema` remains in `parameters`
- after the sanitizer, the regression test passes
- Gemini provider tests pass: `21 passed`
- GUI binary builds successfully
- a manual Gemini run with the patched binary no longer shows the HTTP 400 schema validation error

## Suggested fix

Sanitize or translate tool schemas in the Gemini provider before assigning them to `functionDeclarations[].parameters`. At minimum, filter to Gemini-supported `Schema` fields and remove numeric enum arrays from non-string types.
