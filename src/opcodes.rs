// Core opcode constants used by the simple educational EVM.

// 0x00 range - arithmetic/stop
pub const STOP: u8 = 0x00;
pub const ADD: u8 = 0x01;
pub const MUL: u8 = 0x02;
pub const SUB: u8 = 0x03;
pub const DIV: u8 = 0x04;
// logical/bitwise
pub const LT: u8 = 0x10;
pub const GT: u8 = 0x11;
pub const EQ: u8 = 0x14;
pub const ISZERO: u8 = 0x15;
pub const AND: u8 = 0x16;
pub const OR: u8 = 0x17;
pub const XOR: u8 = 0x18;
pub const NOT: u8 = 0x19;
// SHA3
pub const SHA3: u8 = 0x20;

// 0x50 range - stack/memory/storage/flow
pub const POP: u8 = 0x50;
pub const MLOAD: u8 = 0x51;
pub const MSTORE: u8 = 0x52;
pub const MSTORE8: u8 = 0x53;
pub const SLOAD: u8 = 0x54;
pub const SSTORE: u8 = 0x55;
pub const JUMP: u8 = 0x56;
pub const JUMPI: u8 = 0x57;
pub const JUMPDEST: u8 = 0x5B;
pub const PUSH0: u8 = 0x5F; // Shanghai

// 0x60..0x7f - PUSH1..PUSH32
pub const PUSH1: u8 = 0x60; // start
pub const PUSH32: u8 = 0x7F; // end

// 0x80..0x8f - DUP1..DUP16
pub const DUP1: u8 = 0x80;
pub const DUP16: u8 = 0x8F;

// 0x90..0x9f - SWAP1..SWAP16
pub const SWAP1: u8 = 0x90;
pub const SWAP16: u8 = 0x9F;
