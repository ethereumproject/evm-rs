#![allow(dead_code)]

use inkwell::context::Context;
use inkwell::builder::Builder;
use inkwell::module::Module;

use evmjit::compiler::gas_cost::BasicBlockGasManager;
use evmjit::compiler::runtime::RuntimeManager;
use patch::Patch;
use super::mem_representation::MemoryRepresentation;

pub struct EvmMemory<'a, P: Patch + 'a> {
    m_context: &'a Context,
    m_builder: &'a Builder,
    m_module: &'a Module,
    m_gas_mgr: &'a BasicBlockGasManager<'a, P>,
    m_linear_memory: MemoryRepresentation<'a>
}

impl<'a, P: Patch> EvmMemory<'a, P> {
    pub fn new(context: &'a Context, builder: &'a Builder, module: &'a Module,
               gas_manager: &'a BasicBlockGasManager<'a, P>, rt_manager: &RuntimeManager<'a>) -> EvmMemory<'a, P> {

        let mem = MemoryRepresentation::new(rt_manager.get_mem_ptr(), context, builder, module);

        EvmMemory {
            m_context: context,
            m_builder: builder,
            m_module: module,
            m_gas_mgr: gas_manager,
            m_linear_memory: mem
        }

    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use evmjit::compiler::evm_compiler::MainFuncCreator;
    use evmjit::compiler::external_declarations::ExternalFunctionManager;
    use patch::EmbeddedPatch;

    #[test]
    fn test_memory_creation() {
        let context = Context::create();
        let module = context.create_module("my_module");
        let builder = context.create_builder();
        let decl_factory = ExternalFunctionManager::new(&context, &module);

        //let attr_factory = LLVMAttributeFactory::get_instance(&context);

        // Generate outline of main function needed by 'RuntimeTypeManager
        MainFuncCreator::new("main", &context, &builder, &module);

        let rt_manager = RuntimeManager::new(&context, &builder, &module, &decl_factory);

        let gas_manager : BasicBlockGasManager<EmbeddedPatch> = BasicBlockGasManager::new(&context, &builder, &module, &rt_manager);
        let _memory:EvmMemory<EmbeddedPatch> = EvmMemory::new(&context, &builder, &module, &gas_manager, &rt_manager);
        module.print_to_stderr()
    }
}