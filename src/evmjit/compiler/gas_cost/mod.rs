#![allow(dead_code)]

pub mod fixed_gas_cost;
pub mod variable_gas_cost;

use patch::Patch;
use util::opcode::Opcode;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::builder::Builder;
use inkwell::values::BasicValueEnum;
use inkwell::values::PointerValue;
use inkwell::values::IntValue;
use evmjit::compiler::runtime::RuntimeManager;
use self::variable_gas_cost::VariableGasCostCalculator;

pub trait InstructionGasCost {
    fn count_fixed_instruction_cost(&self, inst_opcode: Opcode) ;

    fn count_variable_cost(&self, cost: BasicValueEnum,
                          exception_dest: Option<PointerValue>, gas_ptr: Option<PointerValue>);
    // Variable cost methods

    fn count_exp_cost(&self, cost: BasicValueEnum);
    fn count_log_data_cost(&self, cost: BasicValueEnum);
    fn count_sha3_data_cost(&self, cost: BasicValueEnum);
    fn copy_data_cost(&self, copy_length: IntValue);
    fn refund_gas(&self, gas_to_refund: IntValue);
}


pub trait BasicBlockGasCost: InstructionGasCost {
    fn finalize_block_cost();

}


pub struct BasicBlockGasManager<'a, P: Patch> {
    m_builder: &'a Builder,
    m_runtime: &'a RuntimeManager<'a>,
    m_variable_cost: VariableGasCostCalculator<'a, P>,
}


impl<'a, P: Patch> BasicBlockGasManager<'a, P> {
    pub fn new(context: &'a Context, builder: &'a Builder, module: &'a Module,
               runtime: &'a RuntimeManager<'a>) -> BasicBlockGasManager<'a, P> {

        let variable_cost_calculator = VariableGasCostCalculator::new(&context, &builder, &module);

        BasicBlockGasManager {
            m_builder: builder,
            m_runtime: runtime,
            m_variable_cost: variable_cost_calculator

        }

    }


}


//impl InstructionGasCost for BasicBlockManager
