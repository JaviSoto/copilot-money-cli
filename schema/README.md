# Schema

Copilot’s GraphQL endpoint disables introspection, so we maintain a **best-effort local schema** for tooling (validation, typed clients/codegen).

## Regenerating

1. Capture operations (writes under `artifacts/graphql-ops/<timestamp>/graphql/`):
   - `python3 tools/capture_graphql_ops.py`
2. Generate/update the stub schema from the newest capture:
   - `cargo run --bin schema-gen -- --out schema/schema.graphql`

The generated schema is incomplete/approximate (e.g., list/non-null shapes and scalar types may be `JSON` until we learn more).

