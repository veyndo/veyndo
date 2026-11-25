# kit/api/generated/operations

Generated native operation contracts; do not edit.

Owner: the compiled operation registry (`veyndo --operation-contracts`, declared by
the components' capability catalogue). Regenerate with
`python3 scripts/testing/operation_contracts.py` after `cargo build`; CI uses `--check`.
No permissions are granted by these types; the native dispatcher validates schemas
and current rights.

## Modules

- `inputs-*.ts`: Input DTOs per operation, twelve operations per file.
- `index.ts`: `OperationName`, `OperationInputs`, `OperationCall` and the declared `OperationOutputs`.
- `value.ts`: `JsonValue` for deliberately open fields.
