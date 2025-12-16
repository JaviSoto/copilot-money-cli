# Schema

Copilot’s GraphQL endpoint disables introspection, so we maintain a **best-effort local schema** for tooling (validation, typed clients/codegen).

## Regenerating

1. Capture operations (writes under `artifacts/graphql-ops/<timestamp>/graphql/`):
   - `python3 tools/capture_graphql_ops.py`
2. Copy selected `.graphql` operation documents into `graphql/` (checked into the repo).
3. Generate/update the stub schema:
   - `cargo run --bin schema-gen -- --out schema/schema.graphql`

To generate directly from the newest capture dir (without copying into `graphql/`), use:

- `cargo run --bin schema-gen -- --latest-artifacts --out schema/schema.graphql`

The generated schema is incomplete/approximate (e.g., list/non-null shapes and scalar types may be `JSON` until we learn more).
