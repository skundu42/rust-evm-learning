use std::collections::{HashMap, HashSet};

use primitive_types::U256;
use thiserror::Error;

use crate::opcodes::*;

#[derive(Debug, Clone)]
pub struct EvmConfig {
    pub gas_limit: i128,
}

impl Default for EvmConfig {
    fn default() -> Self {
        Self { gas_limit: 10_000_000 }
    }
}

#[derive(Debug, Error)]
pub enum EvmError {
    #[error("out of gas")] 
    OutOfGas,
    #[error("stack underflow")] 
    StackUnderflow,
    #[error("stack overflow")] 
    StackOverflow,
    #[error("invalid jump destination {0}")] 
    InvalidJump(usize),
    #[error("invalid opcode 0x{0:02x} at pc={1}")] 
    InvalidOpcode(u8, usize),
    #[error("memory access out of bounds")] 
    MemoryAccess,
}

const STACK_LIMIT: usize = 1024;

#[derive(Debug, Clone)]
pub struct Evm {
    pub pc: usize,
    pub gas: i128,
    pub code: Vec<u8>,
    pub stack: Vec<U256>,
    pub memory: Vec<u8>,
    pub storage: HashMap<U256, U256>,
    jumpdests: HashSet<usize>,
}

impl Evm {
    pub fn new(code: Vec<u8>, cfg: EvmConfig) -> Self {
        let jumpdests = scan_jumpdests(&code);
        Self {
            pc: 0,
            gas: cfg.gas_limit,
            code,
            stack: Vec::with_capacity(64),
            memory: Vec::new(),
            storage: HashMap::new(),
            jumpdests,
        }
    }

    pub fn run(&mut self) -> Result<(), EvmError> {
        while self.pc < self.code.len() {
            let op = self.code[self.pc];
            if op == STOP { break; }
            self.step()?;
        }
        Ok(())
    }

    pub fn step(&mut self) -> Result<(), EvmError> {
        if self.gas <= 0 { return Err(EvmError::OutOfGas); }
        let op = self.code[self.pc];
        match op {
            STOP => { self.pc += 1; }

            // Arithmetic
            ADD => { self.binop(|a,b| a.overflowing_add(b).0); self.gas_dec(3)?; self.pc += 1; }
            MUL => { self.binop(|a,b| a.overflowing_mul(b).0); self.gas_dec(5)?; self.pc += 1; }
            SUB => { self.binop(|a,b| a.overflowing_sub(b).0); self.gas_dec(3)?; self.pc += 1; }
            DIV => { self.binop(|a,b| if b.is_zero() { U256::zero() } else { a / b }); self.gas_dec(5)?; self.pc += 1; }

            // Logic/compare
            LT => { self.binop(|a,b| if a < b { U256::one() } else { U256::zero() }); self.gas_dec(3)?; self.pc += 1; }
            GT => { self.binop(|a,b| if a > b { U256::one() } else { U256::zero() }); self.gas_dec(3)?; self.pc += 1; }
            EQ => { self.binop(|a,b| if a == b { U256::one() } else { U256::zero() }); self.gas_dec(3)?; self.pc += 1; }
            ISZERO => { self.unop(|a| if a.is_zero() { U256::one() } else { U256::zero() }); self.gas_dec(3)?; self.pc += 1; }
            AND => { self.binop(|a,b| a & b); self.gas_dec(3)?; self.pc += 1; }
            OR  => { self.binop(|a,b| a | b); self.gas_dec(3)?; self.pc += 1; }
            XOR => { self.binop(|a,b| a ^ b); self.gas_dec(3)?; self.pc += 1; }
            NOT => { self.unop(|a| !a); self.gas_dec(3)?; self.pc += 1; }

            // Keccak-256
            SHA3 => {
                let offset = self.pop()?;
                let size = self.pop()?;
                let offset_usize = u256_to_usize(offset);
                let size_usize = u256_to_usize(size);
                self.ensure_memory(offset_usize + size_usize);
                let slice = &self.memory[offset_usize..offset_usize + size_usize];
                let mut out = [0u8; 32];
                use tiny_keccak::{Hasher, Keccak};
                let mut hasher = Keccak::v256();
                hasher.update(slice);
                hasher.finalize(&mut out);
                self.push(U256::from_big_endian(&out))?;
                self.gas_dec(30 + (size_usize as i128 + 31) as i128 / 32)?; // rough
                self.pc += 1;
            }

            // Stack/Memory/Storage
            POP => { self.pop()?; self.gas_dec(2)?; self.pc += 1; }
            MLOAD => {
                let offset = self.pop()?; let o = u256_to_usize(offset);
                self.ensure_memory(o + 32);
                let mut buf = [0u8;32];
                buf.copy_from_slice(&self.memory[o..o+32]);
                let val = U256::from_big_endian(&buf);
                self.push(val)?;
                self.gas_dec(3)?;
                self.pc += 1;
            }
            MSTORE => {
                let offset = self.pop()?; let val = self.pop()?; let o = u256_to_usize(offset);
                self.ensure_memory(o + 32);
                let mut buf = [0u8;32];
                val.to_big_endian(&mut buf);
                self.memory[o..o+32].copy_from_slice(&buf);
                self.gas_dec(3)?;
                self.pc += 1;
            }
            MSTORE8 => {
                let offset = self.pop()?; let val = self.pop()?; let o = u256_to_usize(offset);
                self.ensure_memory(o + 1);
                self.memory[o] = (val.low_u32() & 0xFF) as u8;
                self.gas_dec(3)?;
                self.pc += 1;
            }
            SLOAD => {
                let key = self.pop()?;
                let val = *self.storage.get(&key).unwrap_or(&U256::zero());
                self.push(val)?;
                self.gas_dec(100)?; // very rough
                self.pc += 1;
            }
            SSTORE => {
                let key = self.pop()?; let val = self.pop()?;
                self.storage.insert(key, val);
                self.gas_dec(100)?; // very rough
                self.pc += 1;
            }

            // Flow
            JUMP => {
                let dest = self.pop()?; let d = u256_to_usize(dest);
                if !self.jumpdests.contains(&d) { return Err(EvmError::InvalidJump(d)); }
                self.gas_dec(8)?;
                self.pc = d;
            }
            JUMPI => {
                let dest = self.pop()?; let cond = self.pop()?;
                if !cond.is_zero() {
                    let d = u256_to_usize(dest);
                    if !self.jumpdests.contains(&d) { return Err(EvmError::InvalidJump(d)); }
                    self.pc = d;
                } else {
                    self.pc += 1;
                }
                self.gas_dec(10)?;
            }
            JUMPDEST => { self.gas_dec(1)?; self.pc += 1; }

            // PUSH
            x if x == PUSH0 => { self.push(U256::zero())?; self.gas_dec(2)?; self.pc += 1; }
            x if x >= PUSH1 && x <= PUSH32 => {
                let n = (x - PUSH1 + 1) as usize;
                let start = self.pc + 1;
                let end = start + n;
                let slice = if end <= self.code.len() { &self.code[start..end] } else { &[] };
                let mut buf = [0u8; 32];
                let offset = 32 - slice.len();
                if !slice.is_empty() {
                    buf[offset..].copy_from_slice(slice);
                }
                let val = U256::from_big_endian(&buf);
                self.push(val)?;
                self.gas_dec(3)?;
                self.pc = end;
            }

            // DUP
            x if x >= DUP1 && x <= DUP16 => {
                let n = (x - DUP1 + 1) as usize;
                if self.stack.len() < n { return Err(EvmError::StackUnderflow); }
                let val = *self.stack.get(self.stack.len() - n).unwrap();
                self.push(val)?;
                self.gas_dec(3)?;
                self.pc += 1;
            }

            // SWAP
            x if x >= SWAP1 && x <= SWAP16 => {
                let n = (x - SWAP1 + 1) as usize;
                if self.stack.len() < n + 1 { return Err(EvmError::StackUnderflow); }
                let top = self.stack.len() - 1;
                let other = top - n;
                self.stack.swap(top, other);
                self.gas_dec(3)?;
                self.pc += 1;
            }

            _ => return Err(EvmError::InvalidOpcode(op, self.pc)),
        }
        Ok(())
    }

    fn push(&mut self, v: U256) -> Result<(), EvmError> {
        if self.stack.len() >= STACK_LIMIT { return Err(EvmError::StackOverflow); }
        self.stack.push(v);
        Ok(())
    }

    fn pop(&mut self) -> Result<U256, EvmError> {
        self.stack.pop().ok_or(EvmError::StackUnderflow)
    }

    fn binop<F: Fn(U256, U256) -> U256>(&mut self, f: F) {
        let b = self.stack.pop().unwrap_or_else(U256::zero);
        let a = self.stack.pop().unwrap_or_else(U256::zero);
        self.stack.push(f(a, b));
    }

    fn unop<F: Fn(U256) -> U256>(&mut self, f: F) {
        let a = self.stack.pop().unwrap_or_else(U256::zero);
        self.stack.push(f(a));
    }

    fn ensure_memory(&mut self, size: usize) {
        if self.memory.len() < size {
            self.memory.resize(size, 0u8);
        }
    }

    fn gas_dec(&mut self, amount: i128) -> Result<(), EvmError> {
        self.gas -= amount.max(0);
        if self.gas < 0 { Err(EvmError::OutOfGas) } else { Ok(()) }
    }
}

fn scan_jumpdests(code: &[u8]) -> HashSet<usize> {
    let mut set = HashSet::new();
    let mut pc = 0usize;
    while pc < code.len() {
        let op = code[pc];
        if op == JUMPDEST {
            set.insert(pc);
            pc += 1;
        } else if op >= PUSH1 && op <= PUSH32 {
            let n = (op - PUSH1 + 1) as usize;
            pc += 1 + n;
        } else if op == PUSH0 {
            pc += 1;
        } else {
            pc += 1;
        }
    }
    set
}

fn u256_to_usize(v: U256) -> usize {
    // Clamp to usize; this is fine for toy implementation.
    let low = v.low_u128();
    if usize::BITS <= 64 {
        (low as u64) as usize
    } else {
        low as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_add() {
        // PUSH1 0x42; PUSH1 0xFF; ADD
        let code = vec![0x60, 0x42, 0x60, 0xFF, 0x01];
        let mut evm = Evm::new(code, EvmConfig::default());
        evm.run().unwrap();
        assert_eq!(evm.stack.len(), 1);
        assert_eq!(evm.stack[0], U256::from(0x42u64 + 0xFFu64));
    }

    #[test]
    fn push32_and_pop() {
        // PUSH32 0x01.. then POP
        let mut code = vec![0x7f];
        code.extend(std::iter::repeat(0u8).take(31));
        code.push(1);
        code.push(0x50); // POP
        let mut evm = Evm::new(code, EvmConfig::default());
        evm.run().unwrap();
        assert!(evm.stack.is_empty());
    }
}
