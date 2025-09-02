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

// 0x30..3f - environmental (subset)
pub const ADDRESS: u8 = 0x30;
pub const BALANCE: u8 = 0x31;
pub const ORIGIN: u8 = 0x32;
pub const CALLER: u8 = 0x33;
pub const CALLVALUE: u8 = 0x34;
pub const CALLDATALOAD: u8 = 0x35;
pub const CALLDATASIZE: u8 = 0x36;
pub const CALLDATACOPY: u8 = 0x37;
pub const CODESIZE: u8 = 0x38;
pub const CODECOPY: u8 = 0x39;
pub const GASPRICE: u8 = 0x3A;
pub const EXTCODESIZE: u8 = 0x3B;
pub const EXTCODECOPY: u8 = 0x3C;
pub const RETURNDATASIZE: u8 = 0x3D;
pub const RETURNDATACOPY: u8 = 0x3E;
pub const EXTCODEHASH: u8 = 0x3F;

// 0x40..4f - block (subset)
pub const BLOCKHASH: u8 = 0x40;
pub const COINBASE: u8 = 0x41;
pub const TIMESTAMP: u8 = 0x42;
pub const NUMBER: u8 = 0x43;
pub const DIFFICULTY_PRAND: u8 = 0x44; // historical DIFFICULTY / prevrandao
pub const GASLIMIT_OP: u8 = 0x45; // avoid gaslimit name clash
pub const CHAINID: u8 = 0x46;
pub const SELFBALANCE: u8 = 0x47;
pub const BASEFEE: u8 = 0x48;

// 0x50 range - stack/memory/storage/flow
pub const POP: u8 = 0x50;
pub const MLOAD: u8 = 0x51;
pub const MSTORE: u8 = 0x52;
pub const MSTORE8: u8 = 0x53;
pub const SLOAD: u8 = 0x54;
pub const SSTORE: u8 = 0x55;
pub const JUMP: u8 = 0x56;
pub const JUMPI: u8 = 0x57;
pub const PC: u8 = 0x58;
pub const MSIZE: u8 = 0x59;
pub const GAS: u8 = 0x5A;
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

// 0xf0.. returns
pub const RETURN: u8 = 0xF3;
pub const REVERT: u8 = 0xFD;

// 0xf0.. calls/create (subset)
pub const CREATE: u8 = 0xF0;
pub const CALL: u8 = 0xF1;
pub const CALLCODE: u8 = 0xF2;
pub const STATICCALL: u8 = 0xFA;
pub const DELEGATECALL: u8 = 0xF4;
pub const CREATE2: u8 = 0xF5;
// CREATE/CREATE2 could be added later

// logs
pub const LOG0: u8 = 0xA0;
pub const LOG1: u8 = 0xA1;
pub const LOG2: u8 = 0xA2;
pub const LOG3: u8 = 0xA3;
pub const LOG4: u8 = 0xA4;
