use std::collections::{HashMap, HashSet};

use primitive_types::{H160, U256};
use thiserror::Error;

use crate::opcodes::*;

#[derive(Debug, Clone)]
pub struct EvmConfig {
    pub gas_limit: i128,
    pub calldata: Vec<u8>,
    // Environment and world state (optional for single-contract mode)
    pub address: Option<H160>,
    pub caller: Option<H160>,
    pub origin: Option<H160>,
    pub value: U256,
    pub gas_price: U256,
    pub block: BlockEnv,
    pub world: Option<World>,
}

impl Default for EvmConfig {
    fn default() -> Self {
        Self {
            gas_limit: 10_000_000,
            calldata: Vec::new(),
            address: None,
            caller: None,
            origin: None,
            value: U256::zero(),
            gas_price: U256::zero(),
            block: BlockEnv::default(),
            world: None,
        }
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
    #[error("state modification in static context")] 
    StaticViolation,
}

const STACK_LIMIT: usize = 1024;

#[derive(Debug, Clone)]
pub struct Evm {
    pub pc: usize,
    pub gas: i128,
    pub code: Vec<u8>,
    pub stack: Vec<U256>,
    pub memory: Vec<u8>,
    pub storage: HashMap<U256, U256>, // legacy single-contract storage
    pub calldata: Vec<u8>,
    pub return_data: Vec<u8>,
    pub last_return_data: Vec<u8>,
    pub halted: Option<Halt>,
    pub logs: Vec<LogEntry>,
    pub is_static: bool,
    pub refund: i128,
    // Env/world
    pub address: Option<H160>,
    pub caller: Option<H160>,
    pub origin: Option<H160>,
    pub callvalue: U256,
    pub gas_price: U256,
    pub block: BlockEnv,
    pub world: Option<World>,
    jumpdests: HashSet<usize>,
}

#[derive(Debug, Clone)]
pub enum Halt {
    Stop,
    Return,
    Revert,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub topics: Vec<U256>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct BlockEnv {
    pub coinbase: H160,
    pub timestamp: u64,
    pub number: u64,
    pub gas_limit: U256,
    pub chain_id: U256,
    pub basefee: U256,
}

#[derive(Debug, Clone, Default)]
pub struct Account {
    pub nonce: u64,
    pub balance: U256,
    pub code: Vec<u8>,
    pub storage: HashMap<U256, U256>,
}

#[derive(Debug, Clone, Default)]
pub struct World {
    pub accounts: HashMap<H160, Account>,
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
            calldata: cfg.calldata,
            return_data: Vec::new(),
            last_return_data: Vec::new(),
            halted: None,
            logs: Vec::new(),
            is_static: false,
            refund: 0,
            address: cfg.address,
            caller: cfg.caller,
            origin: cfg.origin,
            callvalue: cfg.value,
            gas_price: cfg.gas_price,
            block: cfg.block,
            world: cfg.world,
            jumpdests,
        }
    }

    pub fn run(&mut self) -> Result<(), EvmError> {
        while self.pc < self.code.len() && self.halted.is_none() {
            self.step()?;
        }
        Ok(())
    }

    pub fn step(&mut self) -> Result<(), EvmError> {
        if self.gas <= 0 { return Err(EvmError::OutOfGas); }
        let op = self.code[self.pc];
        match op {
            STOP => { self.gas_dec(0)?; self.halted = Some(Halt::Stop); self.pc = self.code.len(); }

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

            // Env opcodes
            ADDRESS => { self.push(h160_to_u256(self.address.unwrap_or_default()))?; self.gas_dec(2)?; self.pc += 1; }
            BALANCE => {
                let addr = self.pop()?; let h = u256_to_h160(addr);
                let bal = self.world.as_ref().and_then(|w| w.accounts.get(&h)).map(|a| a.balance).unwrap_or_else(U256::zero);
                self.push(bal)?; self.gas_dec(100)?; self.pc += 1;
            }
            ORIGIN => { self.push(h160_to_u256(self.origin.unwrap_or_default()))?; self.gas_dec(2)?; self.pc += 1; }
            CALLER => { self.push(h160_to_u256(self.caller.unwrap_or_default()))?; self.gas_dec(2)?; self.pc += 1; }
            CALLVALUE => { self.push(self.callvalue)?; self.gas_dec(2)?; self.pc += 1; }
            GASPRICE => { self.push(self.gas_price)?; self.gas_dec(2)?; self.pc += 1; }
            EXTCODESIZE => {
                let addr = self.pop()?; let h = u256_to_h160(addr);
                let sz = self.world.as_ref().and_then(|w| w.accounts.get(&h)).map(|a| a.code.len()).unwrap_or(0);
                self.push(U256::from(sz))?; self.gas_dec(100)?; self.pc += 1;
            }
            EXTCODECOPY => {
                let addr = self.pop()?; let mem_offset = self.pop()?; let code_offset = self.pop()?; let size = self.pop()?;
                let h = u256_to_h160(addr);
                let code = self.world.as_ref().and_then(|w| w.accounts.get(&h)).map(|a| a.code.clone()).unwrap_or_default();
                let m = u256_to_usize(mem_offset); let c = u256_to_usize(code_offset); let s = u256_to_usize(size);
                self.charge_memory(m + s)?; self.ensure_memory(m + s);
                for i in 0..s { self.memory[m + i] = *code.get(c + i).unwrap_or(&0); }
                self.gas_dec(100 + ((s as i128 + 31) / 32))?; self.pc += 1;
            }
            RETURNDATASIZE => { self.push(U256::from(self.last_return_data.len()))?; self.gas_dec(2)?; self.pc += 1; }
            RETURNDATACOPY => {
                let mem_offset = self.pop()?; let data_offset = self.pop()?; let size = self.pop()?;
                let m = u256_to_usize(mem_offset); let d = u256_to_usize(data_offset); let s = u256_to_usize(size);
                self.charge_memory(m + s)?; self.ensure_memory(m + s);
                for i in 0..s { let v = *self.last_return_data.get(d + i).unwrap_or(&0); self.memory[m + i] = v; }
                self.gas_dec(3 + ((s as i128 + 31) / 32))?; self.pc += 1;
            }
            EXTCODEHASH => {
                let addr = self.pop()?; let h = u256_to_h160(addr);
                let code = self.world.as_ref().and_then(|w| w.accounts.get(&h)).map(|a| a.code.clone()).unwrap_or_default();
                use tiny_keccak::{Hasher, Keccak};
                let mut out = [0u8; 32]; let mut hasher = Keccak::v256(); hasher.update(&code); hasher.finalize(&mut out);
                self.push(U256::from_big_endian(&out))?; self.gas_dec(400)?; self.pc += 1;
            }

            // Block env
            BLOCKHASH => { self.push(U256::zero())?; self.gas_dec(20)?; self.pc += 1; }
            COINBASE => { self.push(h160_to_u256(self.block.coinbase))?; self.gas_dec(2)?; self.pc += 1; }
            TIMESTAMP => { self.push(U256::from(self.block.timestamp))?; self.gas_dec(2)?; self.pc += 1; }
            NUMBER => { self.push(U256::from(self.block.number))?; self.gas_dec(2)?; self.pc += 1; }
            DIFFICULTY_PRAND => { self.push(U256::zero())?; self.gas_dec(2)?; self.pc += 1; }
            GASLIMIT_OP => { self.push(self.block.gas_limit)?; self.gas_dec(2)?; self.pc += 1; }
            CHAINID => { self.push(self.block.chain_id)?; self.gas_dec(2)?; self.pc += 1; }
            SELFBALANCE => {
                let bal = self.address.and_then(|a| self.world.as_ref().and_then(|w| w.accounts.get(&a))).map(|ac| ac.balance).unwrap_or_else(U256::zero);
                self.push(bal)?; self.gas_dec(5)?; self.pc += 1;
            }
            BASEFEE => { self.push(self.block.basefee)?; self.gas_dec(2)?; self.pc += 1; }

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
                let val = self.sload(key);
                self.push(val)?;
                self.gas_dec(100)?; // very rough
                self.pc += 1;
            }
            SSTORE => {
                if self.is_static { return Err(EvmError::StaticViolation); }
                let key = self.pop()?; let val = self.pop()?;
                let current = self.sload(key);
                let cost = if current.is_zero() && !val.is_zero() { 20_000 } else if !current.is_zero() && val.is_zero() { 5_000 } else { 2_900 };
                self.gas_dec(cost)?;
                if !current.is_zero() && val.is_zero() { self.refund += 15_000; }
                self.sstore(key, val);
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

            // Introspection
            PC => { self.push(U256::from(self.pc))?; self.gas_dec(2)?; self.pc += 1; }
            MSIZE => { self.push(U256::from(self.memory.len()))?; self.gas_dec(2)?; self.pc += 1; }
            GAS => { let g = if self.gas > 0 { self.gas as u128 } else { 0 }; self.push(U256::from(g))?; self.gas_dec(2)?; self.pc += 1; }

            // Code/Calldata
            CALLDATALOAD => {
                let offset = self.pop()?; let o = u256_to_usize(offset);
                let mut buf = [0u8; 32];
                let end = o.saturating_add(32);
                let slice = if o < self.calldata.len() { &self.calldata[o..self.calldata.len().min(end)] } else { &[] };
                let start = 32 - slice.len();
                if !slice.is_empty() { buf[start..start+slice.len()].copy_from_slice(slice); }
                self.push(U256::from_big_endian(&buf))?;
                self.gas_dec(3)?;
                self.pc += 1;
            }
            CALLDATASIZE => { self.push(U256::from(self.calldata.len()))?; self.gas_dec(2)?; self.pc += 1; }
            CALLDATACOPY => {
                let mem_offset = self.pop()?; let data_offset = self.pop()?; let size = self.pop()?;
                let m = u256_to_usize(mem_offset); let d = u256_to_usize(data_offset); let s = u256_to_usize(size);
                self.charge_memory(m + s)?; self.ensure_memory(m + s);
                for i in 0..s {
                    let v = if d + i < self.calldata.len() { self.calldata[d + i] } else { 0 };
                    self.memory[m + i] = v;
                }
                self.gas_dec(3 + ((s as i128 + 31) / 32))?;
                self.pc += 1;
            }
            CODESIZE => { self.push(U256::from(self.code.len()))?; self.gas_dec(2)?; self.pc += 1; }
            CODECOPY => {
                let mem_offset = self.pop()?; let code_offset = self.pop()?; let size = self.pop()?;
                let m = u256_to_usize(mem_offset); let c = u256_to_usize(code_offset); let s = u256_to_usize(size);
                self.charge_memory(m + s)?; self.ensure_memory(m + s);
                for i in 0..s {
                    let v = if c + i < self.code.len() { self.code[c + i] } else { 0 };
                    self.memory[m + i] = v;
                }
                self.gas_dec(3 + ((s as i128 + 31) / 32))?;
                self.pc += 1;
            }

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

            // RETURN / REVERT
            RETURN => {
                let offset = self.pop()?; let size = self.pop()?;
                let o = u256_to_usize(offset); let s = u256_to_usize(size);
                self.ensure_memory(o + s);
                self.return_data = self.memory[o..o+s].to_vec();
                self.halted = Some(Halt::Return);
                self.gas_dec(0)?;
                self.pc = self.code.len();
            }
            REVERT => {
                let offset = self.pop()?; let size = self.pop()?;
                let o = u256_to_usize(offset); let s = u256_to_usize(size);
                self.ensure_memory(o + s);
                self.return_data = self.memory[o..o+s].to_vec();
                self.halted = Some(Halt::Revert);
                self.gas_dec(0)?;
                self.pc = self.code.len();
            }

            // LOG0..LOG4
            x if x >= LOG0 && x <= LOG4 => {
                if self.is_static { return Err(EvmError::StaticViolation); }
                let n = (x - LOG0) as usize; // topics
                let mstart = self.pop()?; let msize = self.pop()?;
                let mut topics = Vec::with_capacity(n);
                for _ in 0..n { topics.push(self.pop()?); }
                let o = u256_to_usize(mstart); let s = u256_to_usize(msize);
                self.ensure_memory(o + s);
                let data = self.memory[o..o+s].to_vec();
                self.logs.push(LogEntry { topics, data });
                self.gas_dec(8 + (s as i128 + 31) / 32)?; // rough
                self.pc += 1;
            }

            // CALL (simplified)
            CALL => {
                let _gas = self.pop()?; let to = self.pop()?; let value = self.pop()?; let in_off = self.pop()?; let in_sz = self.pop()?; let out_off = self.pop()?; let out_sz = self.pop()?;
                let to_h = u256_to_h160(to);
                let io = u256_to_usize(in_off); let isz = u256_to_usize(in_sz);
                let oo = u256_to_usize(out_off); let osz = u256_to_usize(out_sz);
                self.charge_memory(io + isz)?; self.ensure_memory(io + isz);
                self.charge_memory(oo + osz)?; self.ensure_memory(oo + osz);
                let input = self.memory[io..io+isz].to_vec();
                let mut success = false;
                let mut ret = Vec::new();
                let from_addr = self.address.unwrap_or_default();
                let (forward, base) = call_gas(self.gas, _gas.as_u128(), !value.is_zero());
                self.gas_dec(base as i128)?;
                if let Some(w) = &mut self.world {
                    // balance transfer
                    // snapshot world for potential revert
                    let mut w_clone = w.clone();
                    let from_acc = w_clone.accounts.entry(from_addr).or_default();
                    if from_acc.balance >= value {
                        // precompile hook (identity at 0x0004)
                        if let Some(pc_ret) = precompile(to_h, &input) {
                            ret = pc_ret;
                            success = true;
                        } else {
                            from_acc.balance -= value;
                            let to_acc = w_clone.accounts.entry(to_h).or_default();
                            to_acc.balance += value;
                            let code = to_acc.code.clone();
                            let mut child = Evm::new(code, EvmConfig {
                                gas_limit: (forward + if !value.is_zero() { 2300 } else { 0 }) as i128,
                                calldata: input,
                                address: Some(to_h),
                                caller: Some(from_addr),
                                origin: self.origin,
                                value,
                                gas_price: self.gas_price,
                                block: self.block.clone(),
                                world: Some(w_clone.clone()),
                            });
                            if let Err(_e) = child.run() {
                                success = false;
                            } else {
                                success = !matches!(child.halted, Some(Halt::Revert));
                                ret = child.return_data.clone();
                                if success { if let Some(child_world) = child.world.take() { *w = child_world; } }
                            }
                        }
                    } else { success = false; }
                } else {
                    // no world state; simulate as empty call
                    success = true;
                }
                // write return
                for i in 0..osz { self.memory[oo + i] = *ret.get(i).unwrap_or(&0); }
                self.last_return_data = ret;
                self.push(if success { U256::one() } else { U256::zero() })?;
                self.gas_dec(40)?; self.pc += 1;
            }
            STATICCALL => {
                let _gas = self.pop()?; let to = self.pop()?; let in_off = self.pop()?; let in_sz = self.pop()?; let out_off = self.pop()?; let out_sz = self.pop()?;
                let to_h = u256_to_h160(to);
                let io = u256_to_usize(in_off); let isz = u256_to_usize(in_sz);
                let oo = u256_to_usize(out_off); let osz = u256_to_usize(out_sz);
                self.charge_memory(io + isz)?; self.ensure_memory(io + isz);
                self.charge_memory(oo + osz)?; self.ensure_memory(oo + osz);
                let input = self.memory[io..io+isz].to_vec();
                let mut success = false; let mut ret = Vec::new();
                if let Some(w) = &mut self.world {
                    let from_addr = self.address.unwrap_or_default();
                    if let Some(pc_ret) = precompile(to_h, &input) {
                        ret = pc_ret; success = true;
                    } else {
                        let code = w.accounts.get(&to_h).map(|a| a.code.clone()).unwrap_or_default();
                        let mut child = Evm::new(code, EvmConfig {
                            gas_limit: self.gas,
                            calldata: input,
                            address: Some(to_h),
                            caller: Some(from_addr),
                            origin: self.origin,
                            value: U256::zero(),
                            gas_price: self.gas_price,
                            block: self.block.clone(),
                            world: Some(w.clone()),
                        });
                        child.is_static = true;
                        if let Err(_e) = child.run() { success = false; } else {
                            success = !matches!(child.halted, Some(Halt::Revert));
                            ret = child.return_data.clone();
                            if let Some(child_world) = child.world.take() { *w = child_world; }
                        }
                    }
                } else { success = true; }
                for i in 0..osz { self.memory[oo + i] = *ret.get(i).unwrap_or(&0); }
                self.last_return_data = ret;
                self.push(if success { U256::one() } else { U256::zero() })?;
                self.gas_dec(40)?; self.pc += 1;
            }

            // CALLCODE: code from target, state/address from current
            CALLCODE => {
                let _gas = self.pop()?; let to = self.pop()?; let value = self.pop()?; let in_off = self.pop()?; let in_sz = self.pop()?; let out_off = self.pop()?; let out_sz = self.pop()?;
                let to_h = u256_to_h160(to);
                let io = u256_to_usize(in_off); let isz = u256_to_usize(in_sz);
                let oo = u256_to_usize(out_off); let osz = u256_to_usize(out_sz);
                self.charge_memory(io + isz)?; self.ensure_memory(io + isz);
                self.charge_memory(oo + osz)?; self.ensure_memory(oo + osz);
                let input = self.memory[io..io+isz].to_vec();
                let mut success = false; let mut ret = Vec::new();
                let self_addr = self.address.unwrap_or_default();
                let (forward, base) = call_gas(self.gas, _gas.as_u128(), !value.is_zero());
                self.gas_dec(base as i128)?;
                if let Some(w) = &mut self.world {
                    let mut w_clone = w.clone();
                    if w_clone.accounts.entry(self_addr).or_default().balance >= value {
                        if let Some(pc_ret) = precompile(to_h, &input) { ret = pc_ret; success = true; }
                        else {
                            let code = w_clone.accounts.get(&to_h).map(|a| a.code.clone()).unwrap_or_default();
                            let mut child = Evm::new(code, EvmConfig {
                                gas_limit: (forward + if !value.is_zero() { 2300 } else { 0 }) as i128,
                                calldata: input,
                                address: Some(self_addr),
                                caller: Some(self_addr),
                                origin: self.origin,
                                value,
                                gas_price: self.gas_price,
                                block: self.block.clone(),
                                world: Some(w_clone.clone()),
                            });
                            if let Err(_e) = child.run() { success = false; } else {
                                success = !matches!(child.halted, Some(Halt::Revert));
                                ret = child.return_data.clone();
                                if success { if let Some(child_world) = child.world.take() { *w = child_world; } }
                            }
                        }
                    } else { success = false; }
                } else { success = true; }
                for i in 0..osz { self.memory[oo + i] = *ret.get(i).unwrap_or(&0); }
                self.last_return_data = ret;
                self.push(if success { U256::one() } else { U256::zero() })?;
                self.gas_dec(40)?; self.pc += 1;
            }

            // DELEGATECALL: code at target, storage/address/caller/value inherited
            DELEGATECALL => {
                let _gas = self.pop()?; let to = self.pop()?; let in_off = self.pop()?; let in_sz = self.pop()?; let out_off = self.pop()?; let out_sz = self.pop()?;
                let to_h = u256_to_h160(to);
                let io = u256_to_usize(in_off); let isz = u256_to_usize(in_sz);
                let oo = u256_to_usize(out_off); let osz = u256_to_usize(out_sz);
                self.charge_memory(io + isz)?; self.ensure_memory(io + isz);
                self.charge_memory(oo + osz)?; self.ensure_memory(oo + osz);
                let input = self.memory[io..io+isz].to_vec();
                let mut success = false; let mut ret = Vec::new();
                let (forward, base) = call_gas(self.gas, _gas.as_u128(), false);
                self.gas_dec(base as i128)?;
                if let Some(w) = &mut self.world {
                    let mut w_clone = w.clone();
                    if let Some(pc_ret) = precompile(to_h, &input) { ret = pc_ret; success = true; }
                    else {
                        let code = w_clone.accounts.get(&to_h).map(|a| a.code.clone()).unwrap_or_default();
                        let mut child = Evm::new(code, EvmConfig {
                            gas_limit: forward as i128,
                            calldata: input,
                            address: self.address,
                            caller: self.caller,
                            origin: self.origin,
                            value: self.callvalue,
                            gas_price: self.gas_price,
                            block: self.block.clone(),
                            world: Some(w_clone.clone()),
                        });
                        if let Err(_e) = child.run() { success = false; } else {
                            success = !matches!(child.halted, Some(Halt::Revert));
                            ret = child.return_data.clone();
                            if success { if let Some(child_world) = child.world.take() { *w = child_world; } }
                        }
                    }
                } else { success = true; }
                for i in 0..osz { self.memory[oo + i] = *ret.get(i).unwrap_or(&0); }
                self.last_return_data = ret;
                self.push(if success { U256::one() } else { U256::zero() })?;
                self.gas_dec(40)?; self.pc += 1;
            }

            // CREATE: value, offset, size
            CREATE => {
                if self.is_static { return Err(EvmError::StaticViolation); }
                let value = self.pop()?; let offset = self.pop()?; let size = self.pop()?;
                let o = u256_to_usize(offset); let s = u256_to_usize(size);
                self.charge_memory(o + s)?; self.ensure_memory(o + s);
                let init = self.memory[o..o+s].to_vec();
                let mut success = false; let mut created = H160::zero();
                if let Some(w) = &mut self.world {
                    let from = self.address.unwrap_or_default();
                    let mut w_clone = w.clone();
                    let acc = w_clone.accounts.entry(from).or_default();
                    if acc.balance >= value {
                        let nonce = acc.nonce; acc.nonce = acc.nonce.saturating_add(1);
                        created = create_address(from, nonce);
                        acc.balance -= value; let entry = w_clone.accounts.entry(created).or_default(); entry.balance += value;
                        let mut child = Evm::new(init, EvmConfig {
                            gas_limit: self.gas,
                            calldata: Vec::new(),
                            address: Some(created),
                            caller: Some(from),
                            origin: self.origin,
                            value,
                            gas_price: self.gas_price,
                            block: self.block.clone(),
                            world: Some(w_clone.clone()),
                        });
                        if let Err(_e) = child.run() { success = false; } else {
                            success = !matches!(child.halted, Some(Halt::Revert));
                            if success {
                                let code = child.return_data.clone();
                                if let Some(mut child_world) = child.world.take() {
                                    let e = child_world.accounts.entry(created).or_default(); e.code = code;
                                    *w = child_world;
                                }
                            }
                        }
                    } else { success = false; }
                }
                if success { self.push(h160_to_u256(created))?; } else { self.push(U256::zero())?; }
                self.gas_dec(32000)?; self.pc += 1;
            }

            // CREATE2: value, offset, size, salt
            CREATE2 => {
                if self.is_static { return Err(EvmError::StaticViolation); }
                let value = self.pop()?; let offset = self.pop()?; let size = self.pop()?; let salt = self.pop()?;
                let o = u256_to_usize(offset); let s = u256_to_usize(size);
                self.charge_memory(o + s)?; self.ensure_memory(o + s);
                let init = self.memory[o..o+s].to_vec();
                let mut success = false; let mut created = H160::zero();
                if let Some(w) = &mut self.world {
                    let from = self.address.unwrap_or_default();
                    let mut w_clone = w.clone();
                    let acc = w_clone.accounts.entry(from).or_default();
                    if acc.balance >= value {
                        created = create2_address(from, salt, &init);
                        acc.balance -= value; let entry = w_clone.accounts.entry(created).or_default(); entry.balance += value;
                        let mut child = Evm::new(init, EvmConfig {
                            gas_limit: self.gas,
                            calldata: Vec::new(),
                            address: Some(created),
                            caller: Some(from),
                            origin: self.origin,
                            value,
                            gas_price: self.gas_price,
                            block: self.block.clone(),
                            world: Some(w_clone.clone()),
                        });
                        if let Err(_e) = child.run() { success = false; } else {
                            success = !matches!(child.halted, Some(Halt::Revert));
                            if success {
                                let code = child.return_data.clone();
                                if let Some(mut child_world) = child.world.take() {
                                    let e = child_world.accounts.entry(created).or_default(); e.code = code;
                                    *w = child_world;
                                }
                            }
                        }
                    } else { success = false; }
                }
                if success { self.push(h160_to_u256(created))?; } else { self.push(U256::zero())?; }
                self.gas_dec(32000)?; self.pc += 1;
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

    fn charge_memory(&mut self, size: usize) -> Result<(), EvmError> {
        let before = words(self.memory.len());
        let after = words(size);
        if after > before {
            let cost = mem_cost(after) - mem_cost(before);
            self.gas_dec(cost as i128)?;
        }
        Ok(())
    }

    fn sload(&self, key: U256) -> U256 {
        if let Some(w) = &self.world {
            if let Some(addr) = self.address {
                if let Some(acc) = w.accounts.get(&addr) { return *acc.storage.get(&key).unwrap_or(&U256::zero()); }
            }
        }
        *self.storage.get(&key).unwrap_or(&U256::zero())
    }

    fn sstore(&mut self, key: U256, val: U256) {
        if let Some(w) = &mut self.world {
            if let Some(addr) = self.address {
                let acc = w.accounts.entry(addr).or_default();
                acc.storage.insert(key, val);
                return;
            }
        }
        self.storage.insert(key, val);
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

fn h160_to_u256(a: H160) -> U256 {
    let mut buf = [0u8; 32];
    buf[12..].copy_from_slice(a.as_bytes());
    U256::from_big_endian(&buf)
}

fn u256_to_h160(v: U256) -> H160 {
    let mut buf = [0u8; 32];
    v.to_big_endian(&mut buf);
    H160::from_slice(&buf[12..])
}

fn words(size: usize) -> u64 { ((size as u64) + 31) / 32 }
fn mem_cost(words: u64) -> u64 { 3 * words + (words * words) / 512 }
fn call_gas(available: i128, requested: u128, has_value: bool) -> (u128, u64) {
    // Base cost rough: 700 + 9000 if value
    let base: u64 = 700 + if has_value { 9000 } else { 0 };
    let avail_after_base = if available > (base as i128) { (available as u128) - base as u128 } else { 0 };
    let cap = avail_after_base - (avail_after_base / 64); // 63/64
    let forward = requested.min(cap);
    (forward, base)
}

fn rlp_bytes(b: &[u8]) -> Vec<u8> {
    if b.len() == 1 && b[0] < 0x80 { return vec![b[0]]; }
    let mut out = Vec::new();
    out.push(0x80 + (b.len() as u8));
    out.extend_from_slice(b);
    out
}

fn rlp_u64(n: u64) -> Vec<u8> {
    if n == 0 { return vec![0x80]; }
    let mut buf = Vec::new();
    let mut x = n; let mut tmp = [0u8;8]; let mut i = 8;
    while x > 0 { i -= 1; tmp[i] = (x & 0xff) as u8; x >>= 8; }
    buf.extend_from_slice(&tmp[i..]);
    rlp_bytes(&buf)
}

fn create_address(from: H160, nonce: u64) -> H160 {
    let mut rlp = Vec::new();
    let enc_from = rlp_bytes(from.as_bytes());
    let enc_nonce = rlp_u64(nonce);
    let list_len = enc_from.len() + enc_nonce.len();
    rlp.push(0xC0 + (list_len as u8));
    rlp.extend_from_slice(&enc_from);
    rlp.extend_from_slice(&enc_nonce);
    use tiny_keccak::{Hasher, Keccak};
    let mut out = [0u8;32]; let mut k = Keccak::v256(); k.update(&rlp); k.finalize(&mut out);
    H160::from_slice(&out[12..])
}

fn create2_address(from: H160, salt: U256, init: &[u8]) -> H160 {
    use tiny_keccak::{Hasher, Keccak};
    let mut ih_out = [0u8;32]; let mut ih = Keccak::v256(); ih.update(init); ih.finalize(&mut ih_out);
    let mut out = [0u8;32]; let mut k = Keccak::v256();
    k.update(&[0xff]);
    k.update(from.as_bytes());
    let mut sb = [0u8;32]; salt.to_big_endian(&mut sb); k.update(&sb);
    k.update(&ih_out);
    k.finalize(&mut out);
    H160::from_slice(&out[12..])
}

fn precompile(addr: H160, input: &[u8]) -> Option<Vec<u8>> {
    // Minimal: identity at 0x000...04; others unimplemented
    if addr == H160::from_low_u64_be(4) {
        return Some(input.to_vec());
    }
    None
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
