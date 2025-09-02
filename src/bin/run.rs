use evm_from_scratch::{Evm, EvmConfig};
use std::env;

fn parse_hex(s: &str) -> Option<Vec<u8>> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.len() % 2 != 0 { return None; }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i+2], 16).ok())
        .collect()
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: evm-run <bytecode-hex> [gas]");
        eprintln!("Example: evm-run 0x604260ff01");
        std::process::exit(1);
    }
    let code = parse_hex(&args[1]).unwrap_or_else(|| {
        eprintln!("Invalid hex input");
        std::process::exit(1);
    });
    let gas = if args.len() > 2 { args[2].parse::<i128>().unwrap_or(10_000_000) } else { 10_000_000 };
    let cfg = EvmConfig { gas_limit: gas };
    let mut evm = Evm::new(code, cfg);
    match evm.run() {
        Ok(()) => {
            println!("pc: {}", evm.pc);
            println!("gas left: {}", evm.gas);
            println!("stack size: {}", evm.stack.len());
            if let Some(top) = evm.stack.last() { println!("top: 0x{:x}", top); }
        }
        Err(e) => {
            eprintln!("Execution error: {e}");
            std::process::exit(2);
        }
    }
}

