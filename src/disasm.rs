use crate::opcodes::*;

pub fn disassemble(code: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut pc = 0usize;
    while pc < code.len() {
        let op = code[pc];
        let mut line = format!("{:04x}: ", pc);
        match op {
            STOP => { line.push_str("STOP"); pc += 1; }
            ADD => { line.push_str("ADD"); pc += 1; }
            MUL => { line.push_str("MUL"); pc += 1; }
            SUB => { line.push_str("SUB"); pc += 1; }
            DIV => { line.push_str("DIV"); pc += 1; }
            LT => { line.push_str("LT"); pc += 1; }
            GT => { line.push_str("GT"); pc += 1; }
            EQ => { line.push_str("EQ"); pc += 1; }
            ISZERO => { line.push_str("ISZERO"); pc += 1; }
            AND => { line.push_str("AND"); pc += 1; }
            OR => { line.push_str("OR"); pc += 1; }
            XOR => { line.push_str("XOR"); pc += 1; }
            NOT => { line.push_str("NOT"); pc += 1; }
            SHA3 => { line.push_str("SHA3"); pc += 1; }
            ADDRESS => { line.push_str("ADDRESS"); pc += 1; }
            BALANCE => { line.push_str("BALANCE"); pc += 1; }
            ORIGIN => { line.push_str("ORIGIN"); pc += 1; }
            CALLER => { line.push_str("CALLER"); pc += 1; }
            CALLVALUE => { line.push_str("CALLVALUE"); pc += 1; }
            POP => { line.push_str("POP"); pc += 1; }
            MLOAD => { line.push_str("MLOAD"); pc += 1; }
            MSTORE => { line.push_str("MSTORE"); pc += 1; }
            MSTORE8 => { line.push_str("MSTORE8"); pc += 1; }
            SLOAD => { line.push_str("SLOAD"); pc += 1; }
            SSTORE => { line.push_str("SSTORE"); pc += 1; }
            JUMP => { line.push_str("JUMP"); pc += 1; }
            JUMPI => { line.push_str("JUMPI"); pc += 1; }
            JUMPDEST => { line.push_str("JUMPDEST"); pc += 1; }
            PUSH0 => { line.push_str("PUSH0"); pc += 1; }
            x if x >= PUSH1 && x <= PUSH32 => {
                let n = (x - PUSH1 + 1) as usize;
                let start = pc + 1;
                let end = (start + n).min(code.len());
                let imm = &code[start..end];
                line.push_str(&format!("PUSH{} 0x{}", n, hex(imm)));
                pc = start + n;
            }
            x if x >= DUP1 && x <= DUP16 => {
                let n = (x - DUP1 + 1);
                line.push_str(&format!("DUP{}", n));
                pc += 1;
            }
            x if x >= SWAP1 && x <= SWAP16 => {
                let n = (x - SWAP1 + 1);
                line.push_str(&format!("SWAP{}", n));
                pc += 1;
            }
            PC => { line.push_str("PC"); pc += 1; }
            MSIZE => { line.push_str("MSIZE"); pc += 1; }
            GAS => { line.push_str("GAS"); pc += 1; }
            CALLDATALOAD => { line.push_str("CALLDATALOAD"); pc += 1; }
            CALLDATASIZE => { line.push_str("CALLDATASIZE"); pc += 1; }
            CALLDATACOPY => { line.push_str("CALLDATACOPY"); pc += 1; }
            CODESIZE => { line.push_str("CODESIZE"); pc += 1; }
            CODECOPY => { line.push_str("CODECOPY"); pc += 1; }
            GASPRICE => { line.push_str("GASPRICE"); pc += 1; }
            EXTCODESIZE => { line.push_str("EXTCODESIZE"); pc += 1; }
            EXTCODECOPY => { line.push_str("EXTCODECOPY"); pc += 1; }
            RETURNDATASIZE => { line.push_str("RETURNDATASIZE"); pc += 1; }
            RETURNDATACOPY => { line.push_str("RETURNDATACOPY"); pc += 1; }
            EXTCODEHASH => { line.push_str("EXTCODEHASH"); pc += 1; }
            BLOCKHASH => { line.push_str("BLOCKHASH"); pc += 1; }
            COINBASE => { line.push_str("COINBASE"); pc += 1; }
            TIMESTAMP => { line.push_str("TIMESTAMP"); pc += 1; }
            NUMBER => { line.push_str("NUMBER"); pc += 1; }
            DIFFICULTY_PRAND => { line.push_str("PREVRANDAO"); pc += 1; }
            GASLIMIT_OP => { line.push_str("GASLIMIT"); pc += 1; }
            CHAINID => { line.push_str("CHAINID"); pc += 1; }
            SELFBALANCE => { line.push_str("SELFBALANCE"); pc += 1; }
            BASEFEE => { line.push_str("BASEFEE"); pc += 1; }
            RETURN => { line.push_str("RETURN"); pc += 1; }
            REVERT => { line.push_str("REVERT"); pc += 1; }
            CALL => { line.push_str("CALL"); pc += 1; }
            CALLCODE => { line.push_str("CALLCODE"); pc += 1; }
            STATICCALL => { line.push_str("STATICCALL"); pc += 1; }
            DELEGATECALL => { line.push_str("DELEGATECALL"); pc += 1; }
            CREATE => { line.push_str("CREATE"); pc += 1; }
            CREATE2 => { line.push_str("CREATE2"); pc += 1; }
            LOG0 => { line.push_str("LOG0"); pc += 1; }
            LOG1 => { line.push_str("LOG1"); pc += 1; }
            LOG2 => { line.push_str("LOG2"); pc += 1; }
            LOG3 => { line.push_str("LOG3"); pc += 1; }
            LOG4 => { line.push_str("LOG4"); pc += 1; }
            _ => {
                line.push_str(&format!("0x{:02x}", op));
                pc += 1;
            }
        }
        out.push(line);
    }
    out
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes { s.push_str(&format!("{:02x}", b)); }
    s
}
