use util::opcode::Opcode;

use eval::cost::G_ZERO;
use eval::cost::G_BASE;
use eval::cost::G_VERYLOW;
use eval::cost::G_LOW;
use eval::cost::G_MID;
use eval::cost::G_HIGH;
use eval::cost::G_JUMPDEST;
use eval::cost::G_CREATE;
use eval::cost::G_EXP;
use eval::cost::G_LOG;
use eval::cost::G_LOGTOPIC;
use eval::cost::G_SHA3;
use eval::cost::G_BLOCKHASH;
use eval::cost::G_EXTCODEHASH;
use patch::Patch;
use bigint::Gas;

const NO_FIXED_GAS_COST: u64 = 0;


fn u64_from_gas(gas: Gas) -> u64 {
    gas.as_u64() as u64
}

fn u64_from_usize(num: usize) -> u64 {
    num as u64
}

struct FixedGasCostCalculator;

impl FixedGasCostCalculator {
    // Return gas cost for fixed portion of opcode; some opcodes only have fixed costs
    pub fn gas_cost<P: Patch>(inst_opcode: Opcode) -> u64 {
        match inst_opcode {
            Opcode::CALL | Opcode::CALLCODE | Opcode::DELEGATECALL | Opcode::STATICCALL
            => u64_from_gas (P::gas_call()),

            Opcode::SUICIDE
            => u64_from_gas (P::gas_suicide()),

            Opcode::SSTORE
            => NO_FIXED_GAS_COST,

            Opcode::SHA3
            => u64_from_usize(G_SHA3),

            Opcode::LOG(v)
            => (u64_from_usize (G_LOG) + (u64_from_usize (G_LOGTOPIC) * (u64_from_usize (v)))),

            Opcode::EXTCODECOPY
            => u64_from_gas (P::gas_extcode()),

            Opcode::CALLDATACOPY | Opcode::CODECOPY | Opcode::RETURNDATACOPY
            => u64_from_usize (G_VERYLOW),

            Opcode::EXP
            => u64_from_usize (G_EXP),

            Opcode::CREATE | Opcode::CREATE2
            => u64_from_usize (G_CREATE),

            Opcode::JUMPDEST
            => u64_from_usize (G_JUMPDEST),

            Opcode::SLOAD
            => u64_from_gas (P::gas_sload()),

            Opcode::STOP | Opcode::RETURN | Opcode::REVERT
            => u64_from_usize (G_ZERO),

            Opcode::ADDRESS | Opcode::ORIGIN | Opcode::CALLER |
            Opcode::CALLVALUE | Opcode::CALLDATASIZE | Opcode::RETURNDATASIZE |
            Opcode::CODESIZE | Opcode::GASPRICE | Opcode::COINBASE |
            Opcode::TIMESTAMP | Opcode::NUMBER | Opcode::DIFFICULTY |
            Opcode::GASLIMIT | Opcode::POP | Opcode::PC |
            Opcode::MSIZE | Opcode::GAS
            => u64_from_usize (G_BASE),

            Opcode::ADD | Opcode::SUB | Opcode::NOT | Opcode::LT |
            Opcode::GT | Opcode::SLT | Opcode::SGT | Opcode::EQ |
            Opcode::ISZERO | Opcode::AND | Opcode::OR | Opcode::XOR |
            Opcode::BYTE | Opcode::CALLDATALOAD | Opcode::MLOAD |
            Opcode::MSTORE | Opcode::MSTORE8 | Opcode::PUSH(_) |
            Opcode::DUP(_) | Opcode::SWAP(_) |
            Opcode::SHL | Opcode::SHR | Opcode::SAR
            => u64_from_usize (G_VERYLOW),

            // W_low
            Opcode::MUL | Opcode::DIV | Opcode::SDIV | Opcode::MOD |
            Opcode::SMOD | Opcode::SIGNEXTEND
            => u64_from_usize (G_LOW),

            // W_mid
            Opcode::ADDMOD | Opcode::MULMOD | Opcode::JUMP
            => u64_from_usize (G_MID),

            // W_high
            Opcode::JUMPI
            => u64_from_usize (G_HIGH),

            Opcode::EXTCODESIZE
            => P::gas_extcode().as_u64(),

            Opcode::BALANCE
            => P::gas_balance().as_u64(),

            Opcode::BLOCKHASH
            => u64_from_usize (G_BLOCKHASH),

            Opcode::EXTCODEHASH
            => u64_from_usize (G_EXTCODEHASH),

            _ =>  NO_FIXED_GAS_COST
        }

    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use patch::EmbeddedPatch;

    #[test]
    fn test_fixed_gas_costs_very_low() {
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::ADD), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SUB), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::NOT), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::LT), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::GT), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SLT), u64_from_usize (G_VERYLOW));

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SGT), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::EQ), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::ISZERO), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::AND), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::OR), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::XOR), u64_from_usize (G_VERYLOW));

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::BYTE), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CALLDATALOAD), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::MLOAD), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::MSTORE), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::MSTORE8), u64_from_usize (G_VERYLOW));

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SHL), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SHR), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SAR), u64_from_usize (G_VERYLOW));

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CALLDATACOPY), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CODECOPY), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::RETURNDATACOPY), u64_from_usize (G_VERYLOW));
    }

    #[test]
    fn test_fixed_gas_costs_dup() {
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(1)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(2)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(3)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(4)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(5)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(6)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(7)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(8)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(9)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(10)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(11)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(12)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(13)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(14)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(15)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DUP(16)), u64_from_usize (G_VERYLOW));
    }

    #[test]
    fn test_fixed_gas_costs_swap() {
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(1)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(2)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(3)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(4)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(5)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(6)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(7)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(8)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(9)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(10)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(11)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(12)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(13)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(14)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(15)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SWAP(16)), u64_from_usize (G_VERYLOW));
    }

    #[test]
    fn test_fixed_gas_costs_push() {
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(1)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(2)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(3)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(4)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(5)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(6)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(7)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(8)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(9)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(10)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(11)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(12)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(13)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(14)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(15)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(16)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(17)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(18)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(19)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(20)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(21)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(22)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(23)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(24)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(25)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(26)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(27)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(28)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(29)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(30)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(31)), u64_from_usize (G_VERYLOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PUSH(32)), u64_from_usize (G_VERYLOW));
    }

    #[test]
    fn test_fixed_gas_costs_low() {
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::MUL), u64_from_usize (G_LOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DIV), u64_from_usize (G_LOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SDIV), u64_from_usize (G_LOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::MOD), u64_from_usize (G_LOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SMOD), u64_from_usize (G_LOW));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SIGNEXTEND), u64_from_usize (G_LOW));
    }

    #[test]
    fn test_fixed_gas_costs_mid() {
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::ADDMOD), u64_from_usize (G_MID));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::MULMOD), u64_from_usize (G_MID));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::JUMP), u64_from_usize (G_MID));
    }

    #[test]
    fn test_fixed_gas_costs_high() {
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::JUMPI), u64_from_usize (G_HIGH));
    }

    #[test]
    fn test_fixed_gas_costs_base() {
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::ADDRESS), u64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::ORIGIN), u64_from_usize (G_BASE));

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CALLER), u64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CALLVALUE), u64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CALLDATASIZE), u64_from_usize (G_BASE));

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::RETURNDATASIZE), u64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CODESIZE), u64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::GASPRICE), u64_from_usize (G_BASE));

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::COINBASE), u64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::TIMESTAMP), u64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::NUMBER), u64_from_usize (G_BASE));

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DIFFICULTY), u64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::GASLIMIT), u64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::POP), u64_from_usize (G_BASE));

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::PC), u64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::MSIZE), u64_from_usize (G_BASE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::GAS), u64_from_usize (G_BASE));

    }

    #[test]
    fn test_fixed_gas_costs_zero() {
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::STOP), u64_from_usize (G_ZERO));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::RETURN), u64_from_usize (G_ZERO));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::REVERT), u64_from_usize (G_ZERO));
    }

    #[test]
    fn test_fixed_gas_costs_calls() {
        let call_cost : u64 = EmbeddedPatch::gas_call().as_u64();

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CALL), call_cost);
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CALLCODE), call_cost);
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::DELEGATECALL), call_cost);
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::STATICCALL), call_cost);
    }

    #[test]
    fn test_fixed_gas_costs_logs() {
        let fixed_part1 : u64 = u64_from_usize (G_LOG);
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::LOG(0)), fixed_part1);
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::LOG(1)), fixed_part1 + u64_from_usize (G_LOGTOPIC) * 1);
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::LOG(2)), fixed_part1 + u64_from_usize (G_LOGTOPIC) * 2);
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::LOG(3)), fixed_part1 + u64_from_usize (G_LOGTOPIC) * 3);
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::LOG(4)), fixed_part1 + u64_from_usize (G_LOGTOPIC) * 4);
    }

    #[test]
    fn test_fixed_gas_costs_misc() {
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SHA3), u64_from_usize (G_SHA3));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::EXP), u64_from_usize (G_EXP));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CREATE), u64_from_usize (G_CREATE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::CREATE2), u64_from_usize (G_CREATE));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SUICIDE),
                   u64_from_gas (EmbeddedPatch::gas_suicide()));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SSTORE), NO_FIXED_GAS_COST);
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::EXTCODECOPY),
                   u64_from_gas (EmbeddedPatch::gas_extcode()));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::JUMPDEST), u64_from_usize (G_JUMPDEST));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::SLOAD),
                   u64_from_gas (EmbeddedPatch::gas_sload()));

        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::EXTCODESIZE),
                   u64_from_gas (EmbeddedPatch::gas_extcode()));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::BALANCE),
                   u64_from_gas (EmbeddedPatch::gas_balance()));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::BLOCKHASH), u64_from_usize (G_BLOCKHASH));
        assert_eq!(FixedGasCostCalculator::gas_cost::<EmbeddedPatch>(Opcode::EXTCODEHASH), u64_from_usize (G_EXTCODEHASH));
    }
}