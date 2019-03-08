use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::BasicTypeEnum;
use inkwell::values::FunctionValue;
use evmjit::LLVMAttributeFactory;
use singletonum::Singleton;
use evmjit::compiler::evmtypes::EvmTypes;
use inkwell::module::Linkage::*;
use inkwell::AddressSpace;

static FRAME_ADDRESS_INTRINSIC_NAME: &str = "llvm.frameaddress";
static SETJMP_INTRINSIC_NAME: &str = "llvm.eh.sjlj.setjmp";
static LONGJMP_INTRINSIC_NAME: &str = "llvm.eh.sjlj.longjmp";
static STACK_SAVE_INTRINSIC_NAME: &str = "llvm.stacksave";
static BSWAP_I256_IINTRINSIC_NAME: &str = "llvm.bswap.i256";
static BSWAP_I160_IINTRINSIC_NAME: &str = "llvm.bswap.i160";
static CTLZ_I256_IINTRINSIC_NAME: &str = "llvm.ctlz.i256";

pub enum LLVMIntrinsic {
    Bswap,
    Ctlz,
    FrameAddress,
    LongJmp,
    StackSave,
    SetJmp
}

pub trait LLVMIntrinsicManager {
    fn to_name(&self, arg_type: Option<BasicTypeEnum>) -> &'static str;
    fn get_intrinsic_declaration(&self, context: &Context, module: &Module, 
                                 arg_type: Option<BasicTypeEnum>) -> FunctionValue;
}

impl LLVMIntrinsicManager for LLVMIntrinsic {
    fn to_name(&self, arg_type: Option<BasicTypeEnum>) -> &'static str {
        match self {
            LLVMIntrinsic::Bswap => {
                assert!(arg_type != None);
                let arg = arg_type.unwrap();
                assert!(arg.is_int_type());
                let int_bit_width = arg.into_int_type().get_bit_width();
                assert!(int_bit_width == 160 || int_bit_width == 256);
                match int_bit_width {
                    160 => BSWAP_I160_IINTRINSIC_NAME,
                    256 => BSWAP_I256_IINTRINSIC_NAME,
                    _ => panic!("LLVMIntrinsicManager::to_name: bad integer size for bswap"),
                }
            },

            LLVMIntrinsic::Ctlz => {
                assert!(arg_type != None);
                let arg = arg_type.unwrap();
                assert!(arg.is_int_type());
                let int_bit_width = arg.into_int_type().get_bit_width();
                assert!(int_bit_width == 256);

                CTLZ_I256_IINTRINSIC_NAME
            },

            // No type
            LLVMIntrinsic::FrameAddress => {
                assert!(arg_type == None);
                FRAME_ADDRESS_INTRINSIC_NAME
            },

            // No type
            LLVMIntrinsic::LongJmp => {
                assert!(arg_type == None);
                LONGJMP_INTRINSIC_NAME
            },

            // No type
            LLVMIntrinsic::StackSave => {
                assert!(arg_type == None);
                STACK_SAVE_INTRINSIC_NAME
            },

            // No type
            LLVMIntrinsic::SetJmp => {
                assert!(arg_type == None);
                SETJMP_INTRINSIC_NAME
            }
        }
    }

    fn get_intrinsic_declaration(&self, context: &Context, module: &Module, 
                                 arg_type: Option<BasicTypeEnum>) -> FunctionValue {
        match self {
            LLVMIntrinsic::Bswap => {
                let types_instance = EvmTypes::get_instance(context);
                let bswap_ret_type = types_instance.get_word_type();
                let arg1 = types_instance.get_word_type();
                let bswap_func_type = bswap_ret_type.fn_type(&[arg1.into()], false);
                let bswap_func = module.add_function(self.to_name(arg_type), 
                                                          bswap_func_type, 
                                                          Some(External));

                let attr_factory = LLVMAttributeFactory::get_instance(&context);
                bswap_func.add_attribute(0, *attr_factory.attr_nounwind());
                bswap_func.add_attribute(0, *attr_factory.attr_readnone());
                bswap_func.add_attribute(0, *attr_factory.attr_speculatable());
                bswap_func
            },

            LLVMIntrinsic::Ctlz => {
                let types_instance = EvmTypes::get_instance(context);
                let ctlz_ret_type = types_instance.get_word_type();
                let arg1 = types_instance.get_word_type();
                let arg2 = context.bool_type();
                let ctlz_func_type = ctlz_ret_type.fn_type(&[arg1.into(), arg2.into()], false);
                let ctlz_func = module.add_function(self.to_name(arg_type), 
                                                    ctlz_func_type, 
                                                    Some(External));

                let attr_factory = LLVMAttributeFactory::get_instance(&context);
                ctlz_func.add_attribute(0, *attr_factory.attr_nounwind());
                ctlz_func.add_attribute(0, *attr_factory.attr_readnone());
                ctlz_func.add_attribute(0, *attr_factory.attr_speculatable());
                ctlz_func
            },

            // No type
            LLVMIntrinsic::FrameAddress => {
                let frame_addr_ret_type = context.i8_type().ptr_type(AddressSpace::Generic);
                let arg1 = context.i32_type();
                let frame_addr_func_type = frame_addr_ret_type.fn_type(&[arg1.into()], false);
                let frame_addr_func = module.add_function(self.to_name(arg_type), 
                                                          frame_addr_func_type, 
                                                          Some(External));

                let attr_factory = LLVMAttributeFactory::get_instance(&context);
                frame_addr_func.add_attribute(0, *attr_factory.attr_nounwind());
                frame_addr_func.add_attribute(0, *attr_factory.attr_readnone());
                frame_addr_func
            },

            // No type
            LLVMIntrinsic::LongJmp => {
                let longjmp_ret_type = context.void_type();
                let arg1 = context.i8_type().ptr_type(AddressSpace::Generic);
                let longjmp_func_type = longjmp_ret_type.fn_type(&[arg1.into()], false);
                let longjmp_func = module.add_function(self.to_name(arg_type), 
                                                          longjmp_func_type, 
                                                          Some(External));

                let attr_factory = LLVMAttributeFactory::get_instance(&context);
                longjmp_func.add_attribute(0, *attr_factory.attr_nounwind());
                longjmp_func.add_attribute(0, *attr_factory.attr_noreturn());
                longjmp_func
            },

            // No type
            LLVMIntrinsic::StackSave => {
                let stack_save_ret_type = context.i8_type().ptr_type(AddressSpace::Generic);
                let stack_save_func_type = stack_save_ret_type.fn_type(&[], false);
                let stack_save_func = module.add_function(self.to_name(arg_type), 
                                                          stack_save_func_type, 
                                                          Some(External));

                let attr_factory = LLVMAttributeFactory::get_instance(&context);
                stack_save_func.add_attribute(0, *attr_factory.attr_nounwind());
                stack_save_func
            },

            // No type
            LLVMIntrinsic::SetJmp => {
                let setjmp_ret_type = context.i32_type();
                let arg1 = context.i8_type().ptr_type(AddressSpace::Generic);
                let setjmp_func_type = setjmp_ret_type.fn_type(&[arg1.into()], false);
                let setjmp_func = module.add_function(self.to_name(arg_type), 
                                                          setjmp_func_type, 
                                                          Some(External));

                let attr_factory = LLVMAttributeFactory::get_instance(&context);
                setjmp_func.add_attribute(0, *attr_factory.attr_nounwind());
                setjmp_func
            }
        }
    }
}

