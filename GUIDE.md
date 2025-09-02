# EVM From Scratch (Rust) — Detailed Guide

This guide explains the functionality of the minimal EVM implemented in `rust/`, with runnable examples using the `evm-run` CLI.

- Project: `rust/`
- Binary: `evm-run`
- Library: `evm_from_scratch`

Use `cargo run --bin evm-run -- <hex>` to execute bytecode.

## Components

- Stack: 1024-item limit of 256-bit words (`U256`). Arithmetic and logic operate on stack top items.
- Memory: Byte-addressed, zero-initialized, expands on demand. `MSTORE`/`MLOAD` work with 32-byte words (big-endian).
- Storage: Simple key-value map `U256 -> U256` for `SLOAD`/`SSTORE`.
- Gas: Integer budget decremented per operation. Includes rough memory expansion cost and more realistic `SSTORE` costs/refunds.
- PC: Program counter (byte index into `code`).
- Jumpdest scanning: Valid jump targets are precomputed (only `JUMPDEST` bytes are allowed).
 - World state (optional): Accounts with balance, code, storage. Enables environment opcodes and cross-contract calls.

## CLI Basics

- Build: `cd rust && cargo build`
- Run: `cd rust && cargo run --bin evm-run -- 0x604260ff01`
- Output includes `pc`, `gas left`, `stack size`, and `top` (top-of-stack in hex, if present).

## Conventions and Operand Order

- Binary ops (e.g., `ADD`): pop `b` then `a`, push `f(a,b)`.
- `MSTORE`: pop `offset` then `value`. Store 32-byte big-endian `value` at `memory[offset..offset+32]`.
- `MLOAD`: pop `offset`, push 32-byte word at `offset`.
- `SSTORE`: pop `key` then `value`; `storage[key] = value`.
- `SLOAD`: pop `key`, push `storage[key]` or `0`.
- `JUMP`: pop `dest` (must be a `JUMPDEST`).
- `JUMPI`: pop `dest`, then `cond`; jump to `dest` if `cond != 0`.
- `SHA3`: pop `offset`, then `size`; Keccak-256 of `memory[offset..offset+size]`.

Note: Pushing values to match these orders often means pushing the value first and the offset/key/dest second so that the latter is on the top of the stack when the opcode executes.

## Opcode Reference with Examples

Each example is a hex-encoded bytecode you can run with:

```
cargo run --bin evm-run -- <hex>
```

### STOP (0x00)
- Semantics: Halts execution.
- Example: `0x00`

### PUSH0 (0x5f) and PUSH1..PUSH32 (0x60..0x7f)
- Semantics: Pushes an immediate value (0..32 bytes) onto the stack.
- Examples:
  - `PUSH0`: `0x5f`
  - `PUSH1 0x2a`: `0x602a`
  - `PUSH2 0x1234`: `0x611234`

### POP (0x50)
- Semantics: Discards top of stack.
- Example: `PUSH1 0x2a; POP` → `0x602a50` leaves an empty stack.

### Arithmetic: ADD, MUL, SUB, DIV (0x01, 0x02, 0x03, 0x04)
- Semantics: Binary arithmetic on two top-most items.
- Example (ADD): `PUSH1 0x42; PUSH1 0xff; ADD` → `0x604260ff01`
  - Expected top: `0x141` (321 decimal)

### Comparisons and Logic: LT, GT, EQ, ISZERO, AND, OR, XOR, NOT
- LT (0x10), GT (0x11), EQ (0x14), ISZERO (0x15)
- AND (0x16), OR (0x17), XOR (0x18), NOT (0x19)
- Examples:
  - `0x6001600210` → `PUSH1 1; PUSH1 2; LT` → top: `0x1` (1 < 2)
  - `0x6001600214` → `PUSH1 1; PUSH1 2; EQ` → top: `0x0`
  - `0x600015` → `PUSH1 0; ISZERO` → top: `0x1`
  - `0x6001600216` → `PUSH1 1; PUSH1 2; AND` → top: `0x0`

### Memory: MSTORE (0x52), MSTORE8 (0x53), MLOAD (0x51)
- MSTORE: pop `offset`, then `value`; store 32-byte big-endian.
- MSTORE8: pop `offset`, then `value`; store low 8 bits at `offset`.
- MLOAD: pop `offset`; push 32-byte word.
- Examples:
  - Store-and-load a small value:
    - `PUSH1 0x2a; PUSH1 0x00; MSTORE; PUSH1 0x00; MLOAD`
    - Hex: `0x602a600052600051`
    - Expected top: `0x2a`
  - Byte store:
    - `PUSH1 0xff; PUSH1 0x10; MSTORE8; PUSH1 0x10; MLOAD`
    - Hex: `0x60ff601053601051`
    - Expected top: `0xff` at the last byte of the 32-byte word; displayed as `0xff` (word with only lowest byte set).

### Storage: SSTORE (0x55), SLOAD (0x54)
- SSTORE: pop `key`, then `value`; write to storage.
- SLOAD: pop `key`; read from storage (0 if absent).
- Example: store `0x2a` at key `0x01`, then load
  - `PUSH1 0x2a; PUSH1 0x01; SSTORE; PUSH1 0x01; SLOAD`
  - Hex: `0x602a600155600154`
  - Expected top: `0x2a`
 - Gas (simplified realistic): 20_000 for 0→nonzero, 5_000 for nonzero→0 (records 15_000 refund), 2_900 for nonzero→nonzero.

### Control Flow: JUMP (0x56), JUMPI (0x57), JUMPDEST (0x5b)
- Only positions containing `JUMPDEST` are valid jump targets.
- JUMP: pop `dest`; set `pc=dest`.
- JUMPI: pop `dest`, then `cond`; jump if `cond != 0`.
- Examples:
  - Unconditional jump to first `JUMPDEST` (index 3):
    - Bytes (with indices): `[0]=60 [1]=03 [2]=56 [3]=5b [4]=60 [5]=2a [6]=00`
    - Hex: `0x6003565b602a00`
    - After running, top: `0x2a`.
  - Conditional jump (cond=1): `PUSH1 1; PUSH1 3; JUMPI; JUMPDEST; PUSH1 0x2a; STOP`
    - Hex: `0x60016003575b602a00`
    - Jumps to index 3 and pushes `0x2a`.

### SHA3 (Keccak-256) (0x20)
- Pop `offset`, then `size`; hash `memory[offset..offset+size]`.
- Example: hash empty slice (Keccak-256 of empty string)
  - `PUSH1 0; PUSH1 0; SHA3`
  - Hex: `0x6000600020`
  - Expected top: `0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470`
- Example: hash 32-byte word stored at 0
  - `PUSH32 0x...deadbeef; PUSH1 0; MSTORE; PUSH1 0; PUSH1 32; SHA3`
  - Sketch hex: `0x7f<32B>6000526000602020` (fill `<32B>` with the 32-byte value)

### Calls and Creation (subset)

- CALL, STATICCALL, CALLCODE, DELEGATECALL supported with simplified gas and value semantics.
- CALL uses the 63/64 rule for forwarded gas and adds a 2300 stipend if value > 0.
- CALLCODE/DELEGATECALL execute code from another account while keeping the caller’s storage/address context.
- CREATE/CREATE2 deploy contracts by running initcode (from memory); the RETURN data becomes the deployed code.
- Address derivation:
  - CREATE: Keccak(RLP([sender, nonce])) last 20 bytes
  - CREATE2: Keccak(0xff || sender || salt || Keccak(initcode)) last 20 bytes

### Precompiles

- Identity precompile at 0x0000000000000000000000000000000000000004 is implemented (returns input).
- Other precompiles are placeholders for now.

### DUP1..DUP16 (0x80..0x8f) and SWAP1..SWAP16 (0x90..0x9f)
- DUPn: duplicate the nth stack item (1=top) to top.
- SWAPn: swap top with nth+1 item.
- Examples:
  - `PUSH1 1; PUSH1 2; DUP2` → `0x6001600281`
    - Stack becomes: [1, 2, 1]
  - `PUSH1 1; PUSH1 2; SWAP1` → `0x6001600290`
    - Stack becomes: [2, 1]

## Gas Model (Simplified)

Gas is decremented per opcode with:
- Memory expansion cost: 3 gas per 32-byte word plus quadratic term (words^2/512) when memory grows.
- Copy operations charge per 32-byte word.
- CALL base costs and 63/64 rule, plus stipend on value transfer.
- SSTORE costs/refunds as above.
Running out of gas results in an error and halts execution. Costs are still educational approximations, not consensus-accurate.

## Errors and Edge Cases

- OutOfGas: gas dropped below zero.
- StackUnderflow/Overflow: not enough items or exceeding 1024 items.
- InvalidOpcode: unknown byte encountered.
- InvalidJump: jump to a non-`JUMPDEST` position.
- MemoryAccess: bounds errors (guarded by automatic expansion for MLOAD/MSTORE paths).

## Tips for Crafting Bytecode

- Compute jump targets by counting bytes: PUSH opcodes consume the immediate bytes following the opcode.
- For `MSTORE`/`SSTORE`/`JUMPI`/`SHA3`, push the non-immediate operand first so the top-of-stack matches the opcode’s pop order (see Conventions section).
- Values are 256-bit; small `PUSH1`/`PUSH2` are promoted to 32-byte big-endian words internally.

## Testing Locally

- Unit tests: `cd rust && cargo test`
- Quick checks with CLI: compose small programs as shown above and verify the `top` value.

## Extending the EVM

- Add opcodes: extend the `match` in `Evm::step` and update gas.
- Improve accuracy: refine gas (cold access, refunds), implement full call semantics, and more environment opcodes.
- Precompiles: extend hooks to support `sha256`, `ripemd160`, bn128 ops, `blake2f`, etc.

## Limitations

- Educational focus; not consensus-accurate.
- World state and gas are approximations (no cold/warm access, incomplete refunds, etc.).
- Precompiles beyond identity are not implemented.

---

If you want more examples mirrored from the book chapters, we can add a curated set of runnable snippets and expected outputs.
