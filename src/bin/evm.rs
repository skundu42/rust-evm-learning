use clap::{ArgAction, Parser, Subcommand};
use evm_in_rust::{disasm, Account, BlockEnv, Evm, EvmConfig, World};
use primitive_types::{H160, U256};
use std::collections::HashMap;

#[derive(Debug, Parser)]
#[command(name = "evm", about = "Educational EVM CLI")] 
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Run EVM bytecode
    Run {
        /// Hex bytecode (e.g., 0x6001600101) or @file
        code: String,
        /// Gas limit
        #[arg(long, default_value_t = 10_000_000)]
        gas: i128,
        /// Calldata as hex
        #[arg(long, default_value = "0x")] 
        calldata: String,
        /// Print full stack
        #[arg(long)]
        dump_stack: bool,
        /// World JSON file (accounts map)
        #[arg(long)]
        world: Option<String>,
        /// Context address (0x..)
        #[arg(long)]
        address: Option<String>,
        /// Msg caller (0x..)
        #[arg(long)]
        caller: Option<String>,
        /// Tx origin (0x..)
        #[arg(long)]
        origin: Option<String>,
        /// Call value (0x.. or decimal)
        #[arg(long, default_value = "0x0")]
        value: String,
        /// Gas price (0x.. or decimal)
        #[arg(long, default_value = "0x0")]
        gas_price: String,
        /// Block coinbase (0x..)
        #[arg(long)]
        coinbase: Option<String>,
        /// Block timestamp (unix seconds)
        #[arg(long)]
        timestamp: Option<u64>,
        /// Block number
        #[arg(long)]
        number: Option<u64>,
        /// Block gas limit (0x.. or decimal)
        #[arg(long)]
        block_gas_limit: Option<String>,
        /// Chain id (0x.. or decimal)
        #[arg(long)]
        chainid: Option<String>,
        /// Basefee (0x.. or decimal)
        #[arg(long)]
        basefee: Option<String>,
        /// Dump final world JSON to stdout or file path
        #[arg(long)]
        dump_world: Option<Option<String>>,
    },
    /// Disassemble bytecode
    Disasm {
        /// Hex bytecode or @file
        code: String,
    },
    /// Step-through trace
    Trace {
        /// Hex bytecode or @file
        code: String,
        /// Calldata as hex
        #[arg(long, default_value = "0x")] 
        calldata: String,
        /// Gas limit
        #[arg(long, default_value_t = 10_000_000)]
        gas: i128,
        /// Max steps
        #[arg(long, default_value_t = 10_000)]
        max_steps: usize,
        /// World JSON file (accounts map)
        #[arg(long)]
        world: Option<String>,
        /// Context address (0x..)
        #[arg(long)]
        address: Option<String>,
        /// Msg caller (0x..)
        #[arg(long)]
        caller: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Run { code, gas, calldata, dump_stack, world, address, caller, origin, value, gas_price, coinbase, timestamp, number, block_gas_limit, chainid, basefee, dump_world } => {
            run_cmd(&code, gas, &calldata, dump_stack, world.as_deref(), address.as_deref(), caller.as_deref(), origin.as_deref(), &value, &gas_price, coinbase.as_deref(), timestamp, number, block_gas_limit.as_deref(), chainid.as_deref(), basefee.as_deref(), dump_world)
        }
        Cmd::Disasm { code } => disasm_cmd(&code),
        Cmd::Trace { code, calldata, gas, max_steps, world, address, caller } => trace_cmd(&code, &calldata, gas, max_steps, world.as_deref(), address.as_deref(), caller.as_deref()),
    }
}

fn run_cmd(
    code_arg: &str,
    gas: i128,
    calldata_hex: &str,
    dump_stack: bool,
    world_path: Option<&str>,
    address_hex: Option<&str>,
    caller_hex: Option<&str>,
    origin_hex: Option<&str>,
    value_str: &str,
    gas_price_str: &str,
    coinbase_hex: Option<&str>,
    ts: Option<u64>,
    num: Option<u64>,
    block_gas_limit_str: Option<&str>,
    chainid_str: Option<&str>,
    basefee_str: Option<&str>,
    dump_world: Option<Option<String>>,
) {
    let code = read_code_arg(code_arg);
    let calldata = parse_hex(calldata_hex).unwrap_or_else(|| die("Invalid calldata hex"));
    let mut cfg = EvmConfig { gas_limit: gas, calldata, ..EvmConfig::default() };
    cfg.address = address_hex.and_then(parse_h160);
    cfg.caller = caller_hex.and_then(parse_h160);
    cfg.origin = origin_hex.and_then(parse_h160);
    cfg.value = parse_u256(value_str).unwrap_or_else(|| die("Invalid --value"));
    cfg.gas_price = parse_u256(gas_price_str).unwrap_or_else(|| die("Invalid --gas-price"));
    if let Some(cb) = coinbase_hex.and_then(parse_h160) { cfg.block.coinbase = cb; }
    if let Some(t) = ts { cfg.block.timestamp = t; }
    if let Some(n) = num { cfg.block.number = n; }
    if let Some(gl) = block_gas_limit_str.and_then(parse_u256) { cfg.block.gas_limit = gl; }
    if let Some(cid) = chainid_str.and_then(parse_u256) { cfg.block.chain_id = cid; }
    if let Some(bf) = basefee_str.and_then(parse_u256) { cfg.block.basefee = bf; }
    if let Some(path) = world_path { cfg.world = Some(load_world(path)); }
    let mut evm = Evm::new(code, cfg);
    match evm.run() {
        Ok(()) => {
            println!("halted: {}", halt_status(&evm));
            if !evm.return_data.is_empty() {
                println!("return: 0x{}", hex(&evm.return_data));
            }
            println!("pc: {}", evm.pc);
            println!("gas left: {}", evm.gas);
            println!("stack size: {}", evm.stack.len());
            if let Some(top) = evm.stack.last() { println!("top: 0x{:x}", top); }
            if dump_stack {
                for (i, v) in evm.stack.iter().rev().enumerate() {
                    println!("[{}] 0x{:x}", i, v);
                }
            }
            if !evm.logs.is_empty() {
                println!("logs: {}", evm.logs.len());
            }
            if let Some(dw) = dump_world.flatten() {
                let json = world_to_json(evm.world.as_ref());
                if let Some(path) = dw.strip_prefix('@') { std::fs::write(path, json).unwrap_or_else(|e| die(&format!("write world: {e}"))); }
                else { println!("{}", json); }
            }
        }
        Err(e) => die(&format!("Execution error: {e}")),
    }
}

fn disasm_cmd(code_arg: &str) {
    let code = read_code_arg(code_arg);
    for line in disasm::disassemble(&code) {
        println!("{}", line);
    }
}

fn trace_cmd(code_arg: &str, calldata_hex: &str, gas: i128, max_steps: usize, world_path: Option<&str>, address_hex: Option<&str>, caller_hex: Option<&str>) {
    let code = read_code_arg(code_arg);
    let calldata = parse_hex(calldata_hex).unwrap_or_else(|| die("Invalid calldata hex"));
    let mut cfg = EvmConfig { gas_limit: gas, calldata, ..EvmConfig::default() };
    cfg.address = address_hex.and_then(parse_h160);
    cfg.caller = caller_hex.and_then(parse_h160);
    if let Some(path) = world_path { cfg.world = Some(load_world(path)); }
    let mut evm = Evm::new(code, cfg);

    let mut steps = 0usize;
    loop {
        if evm.pc >= evm.code.len() || evm.halted.is_some() || steps >= max_steps {
            println!("-- halt: {} --", halt_status(&evm));
            if !evm.return_data.is_empty() {
                println!("return: 0x{}", hex(&evm.return_data));
            }
            println!("gas left: {}", evm.gas);
            break;
        }
        let op = evm.code[evm.pc];
        println!(
            "pc={:04x} op=0x{:02x} {:8} stack={:2} top={} gas={}",
            evm.pc,
            op,
            opcode_name(op),
            evm.stack.len(),
            evm.stack.last().map(|v| format!("0x{:x}", v)).unwrap_or_else(|| "-".to_string()),
            evm.gas,
        );
        if let Err(e) = evm.step() {
            die(&format!("step error: {e}"));
        }
        steps += 1;
    }
}

fn read_code_arg(arg: &str) -> Vec<u8> {
    if let Some(rest) = arg.strip_prefix('@') {
        std::fs::read(rest).unwrap_or_else(|e| die(&format!("Failed to read file: {e}")))
    } else {
        parse_hex(arg).unwrap_or_else(|| die("Invalid code hex"))
    }
}

fn parse_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.is_empty() { return Some(Vec::new()); }
    if s.len() % 2 != 0 { return None; }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i+2], 16).ok())
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes { s.push_str(&format!("{:02x}", b)); }
    s
}

fn die(msg: &str) -> ! { eprintln!("{}", msg); std::process::exit(1); }

fn halt_status(evm: &Evm) -> &'static str {
    match &evm.halted {
        Some(h) => match h { evm_in_rust::machine::Halt::Stop => "STOP", evm_in_rust::machine::Halt::Return => "RETURN", evm_in_rust::machine::Halt::Revert => "REVERT" },
        None => if evm.pc >= evm.code.len() { "EOF" } else { "RUNNING" },
    }
}

fn opcode_name(op: u8) -> &'static str {
    use evm_in_rust::opcodes::*;
    match op {
        STOP => "STOP",
        ADD => "ADD",
        MUL => "MUL",
        SUB => "SUB",
        DIV => "DIV",
        LT => "LT",
        GT => "GT",
        EQ => "EQ",
        ISZERO => "ISZERO",
        AND => "AND",
        OR => "OR",
        XOR => "XOR",
        NOT => "NOT",
        SHA3 => "SHA3",
        POP => "POP",
        MLOAD => "MLOAD",
        MSTORE => "MSTORE",
        MSTORE8 => "MSTORE8",
        SLOAD => "SLOAD",
        SSTORE => "SSTORE",
        JUMP => "JUMP",
        JUMPI => "JUMPI",
        JUMPDEST => "JUMPDEST",
        PUSH0 => "PUSH0",
        PC => "PC",
        MSIZE => "MSIZE",
        GAS => "GAS",
        CALLDATALOAD => "CALLDATALOAD",
        CALLDATASIZE => "CALLDATASIZE",
        CALLDATACOPY => "CALLDATACOPY",
        CODESIZE => "CODESIZE",
        CODECOPY => "CODECOPY",
        RETURN => "RETURN",
        REVERT => "REVERT",
        LOG0 => "LOG0",
        LOG1 => "LOG1",
        LOG2 => "LOG2",
        LOG3 => "LOG3",
        LOG4 => "LOG4",
        x if x >= PUSH1 && x <= PUSH32 => "PUSHn",
        x if x >= DUP1 && x <= DUP16 => "DUPn",
        x if x >= SWAP1 && x <= SWAP16 => "SWAPn",
        _ => "?",
    }
}

fn world_to_json(world: Option<&World>) -> String {
    use serde_json::{json, Value};
    let mut accounts = serde_json::Map::new();
    if let Some(w) = world {
        for (addr, acc) in &w.accounts {
            let mut stor = serde_json::Map::new();
            for (k, v) in &acc.storage {
                stor.insert(format!("0x{:x}", k), Value::String(format!("0x{:x}", v)));
            }
            accounts.insert(
                format!("0x{}", hex(addr.as_bytes())),
                json!({
                    "nonce": acc.nonce,
                    "balance": format!("0x{:x}", acc.balance),
                    "code": format!("0x{}", hex(&acc.code)),
                    "storage": Value::Object(stor),
                }),
            );
        }
    }
    serde_json::to_string_pretty(&json!({"accounts": Value::Object(accounts)})).unwrap()
}

fn parse_h160(s: &str) -> Option<H160> {
    let b = parse_hex(s)?;
    if b.len() != 20 { return None; }
    Some(H160::from_slice(&b))
}

fn parse_u256(s: &str) -> Option<U256> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        let b = parse_hex(&format!("0x{}", hex))?;
        let mut buf = [0u8; 32];
        if b.len() > 32 { return None; }
        buf[32 - b.len()..].copy_from_slice(&b);
        Some(U256::from_big_endian(&buf))
    } else {
        s.parse::<u128>().ok().map(U256::from)
    }
}

fn load_world(path: &str) -> World {
    let txt = std::fs::read_to_string(path).unwrap_or_else(|e| die(&format!("read world: {e}")));
    let v: serde_json::Value = serde_json::from_str(&txt).unwrap_or_else(|e| die(&format!("parse world json: {e}")));
    let mut world = World { accounts: HashMap::new() };
    if let Some(accs) = v.get("accounts").and_then(|x| x.as_object()) {
        for (k, val) in accs {
            let addr = parse_h160(k).unwrap_or_else(|| die("invalid account key"));
            let mut a = Account::default();
            if let Some(bal) = val.get("balance").and_then(|x| x.as_str()).and_then(parse_u256) { a.balance = bal; }
            if let Some(code_str) = val.get("code").and_then(|x| x.as_str()) { a.code = parse_hex(code_str).unwrap_or_else(|| die("invalid account.code")); }
            if let Some(stor) = val.get("storage").and_then(|x| x.as_object()) {
                for (sk, sv) in stor {
                    let k = parse_u256(sk).unwrap_or_else(|| die("invalid storage key"));
                    let v = sv.as_str().and_then(parse_u256).unwrap_or_else(|| die("invalid storage value"));
                    a.storage.insert(k, v);
                }
            }
            world.accounts.insert(addr, a);
        }
    }
    world
}
