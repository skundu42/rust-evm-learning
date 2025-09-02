# evm-from-scratch (Rust)

This folder contains a minimal, educational EVM implemented in Rust.

What you get:

- A small EVM core with stack, memory, storage, gas.
- A subset of core opcodes implemented (STOP, PUSH0..PUSH32, POP, ADD/SUB/MUL/DIV, logical ops, MLOAD/MSTORE/MSTORE8, SLOAD/SSTORE, JUMP/JUMPI/JUMPDEST, DUP1..16, SWAP1..16, SHA3).
- A CLI `evm-run` to execute hex-encoded bytecode and print the resulting state.
- A comprehensive CLI `evm` with subcommands: `run`, `disasm`, `trace`.
  - Supports calldata, environment opcodes, and world state for simple CALL/STATICCALL.
  - Adds CREATE/CREATE2, CALLCODE, DELEGATECALL semantics (simplified), precompile hooks (identity at 0x04), memory expansion gas, basic SSTORE gas/refunds, and call gas with 63/64 rule + stipend.

## Build

Ensure Rust is installed (Rust 1.70+ recommended), then:

```
cargo build
```

## Run

Run a simple example matching the book’s `SIMPLE_ADD` sequence `PUSH1 0x42; PUSH1 0xff; ADD`:

```
cargo run --bin evm-run -- 0x604260ff01
```

You should see the stack contain `0x141` (0x42 + 0xff) at the top.

## New CLI (`evm`)

- Run with calldata and gas:
  - `cargo run --bin evm -- run 0x604260ff01 --gas 100000 --calldata 0x`
- Disassemble bytecode:
  - `cargo run --bin evm -- disasm 0x60016001526000526020600020f3`
- Step trace execution:
  - `cargo run --bin evm -- trace 0x6001600101 --max-steps 16`

### World/Env options

You can pass execution context and a simple world state to exercise environment opcodes and calls:

- Context flags: `--address 0x.. --caller 0x.. --origin 0x.. --value 0x.. --gas-price 0x..`
- Block flags: `--coinbase 0x.. --timestamp N --number N --block-gas-limit 0x.. --chainid 0x.. --basefee 0x..`
- World file: `--world world.json`

World file format (minimal):

```
{
  "accounts": {
    "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa": {
      "balance": "0xde0b6b3a7640000",
      "code": "0x60016000f3", // example code (RETURN 1)
      "storage": { "0x01": "0x02" }
    }
  }
}
```

Example CALL using world state:

```
cargo run --bin evm -- run 0x600160005260016000f1 --world world.json --address 0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
```

Notes: CALL/STATICCALL are simplified (naive gas, basic value transfer, per-account storage). RETURNDATA* and EXTCODE* are supported. CREATE/CREATE2 and full gas/refund rules are not implemented yet.
 
### Contract creation

- CREATE: `PUSH <value>; PUSH <offset>; PUSH <size>; CREATE` (returns new address or 0)
- CREATE2: `PUSH <value>; PUSH <offset>; PUSH <size>; PUSH <salt>; CREATE2`
- Address derivation:
  - CREATE: Keccak(RLP([sender, nonce])) last 20 bytes
  - CREATE2: Keccak(0xff || sender || salt || Keccak(initcode)) last 20 bytes
- Initcode runs in a child EVM and its RETURN data is used as contract code.

### Precompiles

- Identity precompile at address 0x0000000000000000000000000000000000000004 supported (returns input).
- Other precompiles are stubbed (not executed) for now.

### Dumping world

- Use `--dump-world` to print final world JSON (or `--dump-world @path` to write to a file).

Supported additional opcodes include `RETURN`, `REVERT`, `PC`, `MSIZE`, `GAS`, `CALLDATALOAD`, `CALLDATASIZE`, `CALLDATACOPY`, `CODESIZE`, `CODECOPY`, and `LOG0..LOG4` with basic gas accounting. The EVM stores `return_data`, a `halted` status, and collected `logs` for inspection via the CLI.

## Extend

- Add more opcodes by matching on their byte values in `src/machine.rs`.
- Adjust gas costs as needed for accuracy (costs here are indicative only).
- Expand state/account modeling if you want to simulate transactions and contexts.

## Notes

- This is not a consensus-grade implementation. The goal is clarity and alignment with the book’s didactic progression.
- `tiny-keccak` is used for `SHA3` (Keccak-256). Other precompiles are out of scope.

## Detailed Guide

See `GUIDE.md` for a deep dive into features, opcode behavior, and worked examples.
