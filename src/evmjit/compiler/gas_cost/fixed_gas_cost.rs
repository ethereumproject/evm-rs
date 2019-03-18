use util::opcode::Opcode;

use eval::cost::G_ZERO;
use eval::cost::G_BASE;
use eval::cost::G_VERYLOW;
use eval::cost::G_LOW;
use eval::cost::G_MID;
use eval::cost::G_HIGH;
use eval::cost::G_JUMPDEST;
use eval::cost::G_CREATE;
//use eval::cost::G_CODEDEPOSIT;
//use eval::cost::G_CALLVALUE;
//use eval::cost::G_CALLSTIPEND;
//use eval::cost::G_NEWACCOUNT;
use eval::cost::G_EXP;
//use eval::cost::G_MEMORY;
use eval::cost::G_LOG;
//use eval::cost::G_LOGDATA;
use eval::cost::G_LOGTOPIC;
use eval::cost::G_SHA3;
//use eval::cost::G_SHA3WORD;
//use eval::cost::G_COPY;
use eval::cost::G_BLOCKHASH;
use eval::cost::G_EXTCODEHASH;
use patch::Patch;
use bigint::Gas;

const NO_FIXED_GAS_COST: i64 = 0;


fn i64_from_gas(gas: Gas) -> i64 {
    gas.as_u64() as i64
}

fn i64_from_usize(num: usize) -> i64 {
    num as i64
}

struct FixedGasCostCalculator;

impl FixedGasCostCalculator {
    // Return gas cost for fixed portion of opcode; some opcodes only have fixed costs
    pub fn gas_cost<P: Patch>(inst_opcode: Opcode) -> i64 {
        match inst_opcode {
            Opcode::CALL | Opcode::CALLCODE | Opcode::DELEGATECALL | Opcode::STATICCALL
            => i64_from_gas (P::gas_call()),

            Opcode::SUICIDE
            => i64_from_gas (P::gas_suicide()),

            Opcode::SSTORE
            => NO_FIXED_GAS_COST,

            Opcode::SHA3
            => i64_from_usize(G_SHA3),

            Opcode::LOG(v)
            => (i64_from_usize (G_LOG) + (i64_from_usize (G_LOGTOPIC) * (i64_from_usize (v)))),

            Opcode::EXTCODECOPY
            => i64_from_gas (P::gas_extcode()),

            Opcode::CALLDATACOPY | Opcode::CODECOPY | Opcode::RETURNDATACOPY
            => i64_from_usize (G_VERYLOW),

            Opcode::EXP
            => i64_from_usize (G_EXP),

            Opcode::CREATE | Opcode::CREATE2
            => i64_from_usize (G_CREATE),

            Opcode::JUMPDEST
            => i64_from_usize (G_JUMPDEST),

            Opcode::SLOAD
            => i64_from_gas (P::gas_sload()),

            Opcode::STOP | Opcode::RETURN | Opcode::REVERT
            => i64_from_usize (G_ZERO),

            Opcode::ADDRESS | Opcode::ORIGIN | Opcode::CALLER |
            Opcode::CALLVALUE | Opcode::CALLDATASIZE | Opcode::RETURNDATASIZE |
            Opcode::CODESIZE | Opcode::GASPRICE | Opcode::COINBASE |
            Opcode::TIMESTAMP | Opcode::NUMBER | Opcode::DIFFICULTY |
            Opcode::GASLIMIT | Opcode::POP | Opcode::PC |
            Opcode::MSIZE | Opcode::GAS
            => i64_from_usize (G_BASE),

            Opcode::ADD | Opcode::SUB | Opcode::NOT | Opcode::LT |
            Opcode::GT | Opcode::SLT | Opcode::SGT | Opcode::EQ |
            Opcode::ISZERO | Opcode::AND | Opcode::OR | Opcode::XOR |
            Opcode::BYTE | Opcode::CALLDATALOAD | Opcode::MLOAD |
            Opcode::MSTORE | Opcode::MSTORE8 | Opcode::PUSH(_) |
            Opcode::DUP(_) | Opcode::SWAP(_) |
            Opcode::SHL | Opcode::SHR | Opcode::SAR
            => i64_from_usize (G_VERYLOW),

            // W_low
            Opcode::MUL | Opcode::DIV | Opcode::SDIV | Opcode::MOD |
            Opcode::SMOD | Opcode::SIGNEXTEND
            => i64_from_usize (G_LOW),

            // W_mid
            Opcode::ADDMOD | Opcode::MULMOD | Opcode::JUMP
            => i64_from_usize (G_MID),

            // W_high
            Opcode::JUMPI
            => i64_from_usize (G_HIGH),

            Opcode::EXTCODESIZE
            => P::gas_extcode().as_u64() as i64,

            Opcode::BALANCE
            => P::gas_balance().as_u64() as i64,

            Opcode::BLOCKHASH
            => i64_from_usize (G_BLOCKHASH),

            Opcode::EXTCODEHASH
            => i64_from_usize (G_EXTCODEHASH),

            _ =>  NO_FIXED_GAS_COST
        }

    }
}

/*
pub trait GasMeterManager {
    fn meter_instruction_cost(&self, inst_opcode: Opcode);
    fn meter_runtime_cost(&self, cost: BasicValueEnum,
                          exception_dest: Option<PointerValue>, gas_ptr: Option<PointerValue>);
    fn meter_exp_cost(&self, cost: BasicValueEnum);
    fn meter_log_data_cost(&self, cost: BasicValueEnum);
    fn meter_sha3_data_cost(&self, cost: BasicValueEnum);
}
*/


#[cfg(test)]
mod tests {
    use super::*;
    use patch::EmbeddedPatch;

    #[test]
    fn test_fixed_gas_costs_very_low() {
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::ADD), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SUB), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::NOT), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::LT), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::GT), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SLT), i64_from_usize (G_VERYLOW));

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SGT), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::EQ), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::ISZERO), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::AND), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::OR), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::XOR), i64_from_usize (G_VERYLOW));

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::BYTE), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CALLDATALOAD), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::MLOAD), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::MSTORE), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::MSTORE8), i64_from_usize (G_VERYLOW));

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SHL), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SHR), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SAR), i64_from_usize (G_VERYLOW));

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CALLDATACOPY), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CODECOPY), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::RETURNDATACOPY), i64_from_usize (G_VERYLOW));
    }

    #[test]
    fn test_fixed_gas_costs_dup() {
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(1)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(2)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(3)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(4)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(5)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(6)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(7)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(8)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(9)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(10)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(11)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(12)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(13)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(14)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(15)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(16)), i64_from_usize (G_VERYLOW));
    }

    #[test]
    fn test_fixed_gas_costs_swap() {
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(1)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(2)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(3)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(4)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(5)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(6)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(7)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(8)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(9)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(10)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(11)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(12)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(13)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(14)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(15)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(16)), i64_from_usize (G_VERYLOW));
    }

    #[test]
    fn test_fixed_gas_costs_push() {
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(1)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(2)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(3)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(4)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(5)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(6)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(7)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(8)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(9)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(10)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(11)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(12)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(13)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(14)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(15)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(16)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(17)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(18)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(19)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(20)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(21)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(22)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(23)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(24)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(25)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(26)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(27)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(28)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(29)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(30)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(31)), i64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(32)), i64_from_usize (G_VERYLOW));
    }

    #[test]
    fn test_fixed_gas_costs_low() {
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::MUL), i64_from_usize (G_LOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DIV), i64_from_usize (G_LOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SDIV), i64_from_usize (G_LOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::MOD), i64_from_usize (G_LOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SMOD), i64_from_usize (G_LOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SIGNEXTEND), i64_from_usize (G_LOW));
    }

    #[test]
    fn test_fixed_gas_costs_mid() {
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::ADDMOD), i64_from_usize (G_MID));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::MULMOD), i64_from_usize (G_MID));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::JUMP), i64_from_usize (G_MID));
    }

    #[test]
    fn test_fixed_gas_costs_high() {
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::JUMPI), i64_from_usize (G_HIGH));
    }

    #[test]
    fn test_fixed_gas_costs_base() {
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::ADDRESS), i64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::ORIGIN), i64_from_usize (G_BASE));

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CALLER), i64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CALLVALUE), i64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CALLDATASIZE), i64_from_usize (G_BASE));

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::RETURNDATASIZE), i64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CODESIZE), i64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::GASPRICE), i64_from_usize (G_BASE));

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::COINBASE), i64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::TIMESTAMP), i64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::NUMBER), i64_from_usize (G_BASE));

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DIFFICULTY), i64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::GASLIMIT), i64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::POP), i64_from_usize (G_BASE));

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PC), i64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::MSIZE), i64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::GAS), i64_from_usize (G_BASE));

    }

    #[test]
    fn test_fixed_gas_costs_zero() {
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::STOP), i64_from_usize (G_ZERO));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::RETURN), i64_from_usize (G_ZERO));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::REVERT), i64_from_usize (G_ZERO));
    }

    #[test]
    fn test_fixed_gas_costs_calls() {
        //let call_cost = EmbeddedPatch::gas_call() as i64;
        let call_cost : i64 = EmbeddedPatch::gas_call().as_u64() as i64;

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CALL), call_cost);
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CALLCODE), call_cost);
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DELEGATECALL), call_cost);
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::STATICCALL), call_cost);
    }

    #[test]
    fn test_fixed_gas_costs_logs() {
        let fixed_part1 : i64 = G_LOG as i64;
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::LOG(0)), fixed_part1);
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::LOG(1)), fixed_part1 + i64_from_usize (G_LOGTOPIC) * 1);
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::LOG(2)), fixed_part1 + i64_from_usize (G_LOGTOPIC) * 2);
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::LOG(3)), fixed_part1 + i64_from_usize (G_LOGTOPIC) * 3);
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::LOG(4)), fixed_part1 + i64_from_usize (G_LOGTOPIC) * 4);
    }

    #[test]
    fn test_fixed_gas_costs_misc() {
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SHA3), i64_from_usize (G_SHA3));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::EXP), i64_from_usize (G_EXP));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CREATE), i64_from_usize (G_CREATE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CREATE2), i64_from_usize (G_CREATE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SUICIDE),
                   i64_from_gas (EmbeddedPatch::gas_suicide()));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SSTORE), NO_FIXED_GAS_COST);
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::EXTCODECOPY),
                   i64_from_gas (EmbeddedPatch::gas_extcode()));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::JUMPDEST), i64_from_usize (G_JUMPDEST));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SLOAD),
                   i64_from_gas (EmbeddedPatch::gas_sload()));

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::EXTCODESIZE),
                   i64_from_gas (EmbeddedPatch::gas_extcode()));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::BALANCE),
                   i64_from_gas (EmbeddedPatch::gas_balance()));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::BLOCKHASH), i64_from_usize (G_BLOCKHASH));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::EXTCODEHASH), i64_from_usize (G_EXTCODEHASH));
    }
}