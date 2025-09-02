# evm-from-scratch (Rust)

This folder contains a minimal, educational EVM implemented in Rust to mirror the structure and learning flow from the book in this repository.

What you get:

- A small EVM core with stack, memory, storage, gas.
- A subset of core opcodes implemented (STOP, PUSH0..PUSH32, POP, ADD/SUB/MUL/DIV, logical ops, MLOAD/MSTORE/MSTORE8, SLOAD/SSTORE, JUMP/JUMPI/JUMPDEST, DUP1..16, SWAP1..16, SHA3).
- A CLI `evm-run` to execute hex-encoded bytecode and print the resulting state.

This is intentionally compact and approachable. It’s designed as a learning aid and a base for extension alongside the chapters in `content/`.

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

## Extend

- Add more opcodes by matching on their byte values in `src/machine.rs`.
- Adjust gas costs as needed for accuracy (costs here are indicative only).
- Expand state/account modeling if you want to simulate transactions and contexts.

## Notes

- This is not a consensus-grade implementation. The goal is clarity and alignment with the book’s didactic progression.
- `tiny-keccak` is used for `SHA3` (Keccak-256). Other precompiles are out of scope.

## Detailed Guide

See `GUIDE.md` for a deep dive into features, opcode behavior, and worked examples.
