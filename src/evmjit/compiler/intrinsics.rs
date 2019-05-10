use inkwell::module::Linkage::*;
use inkwell::types::BasicTypeEnum;
use inkwell::types::IntType;
use inkwell::values::FunctionValue;
use inkwell::AddressSpace;

use super::JITContext;

static FRAME_ADDRESS_INTRINSIC_NAME: &str = "llvm.frameaddress";
static SETJMP_INTRINSIC_NAME: &str = "llvm.eh.sjlj.setjmp";
static LONGJMP_INTRINSIC_NAME: &str = "llvm.eh.sjlj.longjmp";
static STACK_SAVE_INTRINSIC_NAME: &str = "llvm.stacksave";
static BSWAP_I64_INTRINSIC_NAME: &str = "llvm.bswap.i64";
static BSWAP_I256_INTRINSIC_NAME: &str = "llvm.bswap.i256";
static BSWAP_I160_INTRINSIC_NAME: &str = "llvm.bswap.i160";
static CTLZ_I256_INTRINSIC_NAME: &str = "llvm.ctlz.i256";
static MEMSET_I32_INTRINSIC_NAME: &str = "llvm.memset.p0i8.i32";
static MEMSET_I64_INTRINSIC_NAME: &str = "llvm.memset.p0i8.i64";

pub enum LLVMIntrinsic {
    Bswap,
    Ctlz,
    FrameAddress,
    LongJmp,
    MemSet,
    StackSave,
    SetJmp,
}

pub trait LLVMIntrinsicManager {
    fn to_name(&self, arg_type: Option<BasicTypeEnum>) -> &'static str;
    fn get_intrinsic_declaration(&self, context: &JITContext, arg_type: Option<BasicTypeEnum>) -> FunctionValue;
}

impl LLVMIntrinsicManager for LLVMIntrinsic {
    fn to_name(&self, arg_type: Option<BasicTypeEnum>) -> &'static str {
        match self {
            LLVMIntrinsic::Bswap => {
                assert!(arg_type != None);
                let arg = arg_type.unwrap();
                assert!(arg.is_int_type());
                let int_bit_width = arg.into_int_type().get_bit_width();
                assert!(int_bit_width == 160 || int_bit_width == 256 || int_bit_width == 64);
                match int_bit_width {
                    160 => BSWAP_I160_INTRINSIC_NAME,
                    256 => BSWAP_I256_INTRINSIC_NAME,
                    64 => BSWAP_I64_INTRINSIC_NAME,
                    _ => panic!("LLVMIntrinsicManager::to_name: bad integer size for bswap"),
                }
            }

            LLVMIntrinsic::Ctlz => {
                assert!(arg_type != None);
                let arg = arg_type.unwrap();
                assert!(arg.is_int_type());
                let int_bit_width = arg.into_int_type().get_bit_width();
                assert!(int_bit_width == 256);

                CTLZ_I256_INTRINSIC_NAME
            }

            LLVMIntrinsic::MemSet => {
                assert!(arg_type != None);
                let arg = arg_type.unwrap();
                assert!(arg.is_int_type());
                let int_bit_width = arg.into_int_type().get_bit_width();
                assert!(int_bit_width == 32 || int_bit_width == 64);
                match int_bit_width {
                    32 => MEMSET_I32_INTRINSIC_NAME,
                    64 => MEMSET_I64_INTRINSIC_NAME,
                    _ => panic!("LLVMIntrinsicManager::to_name: bad integer size for memset"),
                }
            }

            // No type
            LLVMIntrinsic::FrameAddress => {
                assert!(arg_type == None);
                FRAME_ADDRESS_INTRINSIC_NAME
            }

            // No type
            LLVMIntrinsic::LongJmp => {
                assert!(arg_type == None);
                LONGJMP_INTRINSIC_NAME
            }

            // No type
            LLVMIntrinsic::StackSave => {
                assert!(arg_type == None);
                STACK_SAVE_INTRINSIC_NAME
            }

            // No type
            LLVMIntrinsic::SetJmp => {
                assert!(arg_type == None);
                SETJMP_INTRINSIC_NAME
            }
        }
    }

    fn get_intrinsic_declaration(&self, context: &JITContext, arg_type: Option<BasicTypeEnum>) -> FunctionValue {
        match self {
            LLVMIntrinsic::Bswap => {
                assert!(arg_type != None);
                assert!(arg_type.unwrap().is_int_type());

                let bswap_func_name = self.to_name(arg_type);
                let module = context.module();

                let bswap_func_found = module.get_function(bswap_func_name);
                if bswap_func_found.is_some() {
                    bswap_func_found.unwrap()
                }
                else {
                    let int_bit_width = arg_type.unwrap().into_int_type().get_bit_width();

                    let bswap_ret_type = context.llvm_context().custom_width_int_type(int_bit_width);

                    let width_t = IntType::custom_width_int_type(int_bit_width);
                    let type_enum = BasicTypeEnum::IntType(width_t);

                    let bswap_func_type = bswap_ret_type.fn_type(&[type_enum.into()], false);
                    let bswap_func = module.add_function(bswap_func_name, bswap_func_type, Some(External));

                    let attr_factory = context.attributes();
                    bswap_func.add_attribute(0, *attr_factory.attr_nounwind());
                    bswap_func.add_attribute(0, *attr_factory.attr_readnone());
                    bswap_func.add_attribute(0, *attr_factory.attr_speculatable());
                    bswap_func
                }
            }

            LLVMIntrinsic::MemSet => {
                assert!(arg_type != None);
                assert!(arg_type.unwrap().is_int_type());

                let memset_func_name = self.to_name(arg_type);
                let module = context.module();


                let memset_func_found = module.get_function(memset_func_name);
                if memset_func_found.is_some() {
                    memset_func_found.unwrap()
                }
                else {
                    let llvm_ctx = context.llvm_context();
                    let int_bit_width = arg_type.unwrap().into_int_type().get_bit_width();

                    let width_t = if int_bit_width == 64 {
                        IntType::i64_type()
                    } else {
                        IntType::i32_type()
                    };

                    let type_enum = BasicTypeEnum::IntType(width_t);
                    let memset_ret_type = llvm_ctx.void_type();
                    let arg1 = llvm_ctx.i8_type().ptr_type(AddressSpace::Generic);
                    let arg2 = llvm_ctx.i8_type();
                    let arg4 = llvm_ctx.bool_type();

                    let memset_func_type =
                        memset_ret_type.fn_type(&[arg1.into(), arg2.into(), type_enum.into(), arg4.into()], false);
                    let memset_func = module.add_function(memset_func_name, memset_func_type, Some(External));
                    let attr_factory = context.attributes();

                    memset_func.add_attribute(0, *attr_factory.attr_nounwind());
                    memset_func.add_attribute(0, *attr_factory.attr_argmemonly());
                    memset_func.add_attribute(1, *attr_factory.attr_nocapture());
                    memset_func
                }
            }

            LLVMIntrinsic::Ctlz => {
                let module = context.module();

                let ctlz_func_name = self.to_name(arg_type);
                let ctlz_func_found = module.get_function(ctlz_func_name);
                if ctlz_func_found.is_some() {
                    ctlz_func_found.unwrap()
                } else {
                    let llvm_ctx = context.llvm_context();
                    let types_instance = context.evm_types();
                    let ctlz_ret_type = types_instance.get_word_type();
                    let arg1 = types_instance.get_word_type();
                    let arg2 = llvm_ctx.bool_type();
                    let ctlz_func_type = ctlz_ret_type.fn_type(&[arg1.into(), arg2.into()], false);
                    let ctlz_func = module.add_function(ctlz_func_name, ctlz_func_type, Some(External));

                    let attr_factory = context.attributes();
                    ctlz_func.add_attribute(0, *attr_factory.attr_nounwind());
                    ctlz_func.add_attribute(0, *attr_factory.attr_readnone());
                    ctlz_func.add_attribute(0, *attr_factory.attr_speculatable());
                    ctlz_func
                }
            }

            // No type
            LLVMIntrinsic::FrameAddress => {
                let module = context.module();

                let frame_addr_func_name = self.to_name(arg_type);
                let frame_addr_func_found = module.get_function(frame_addr_func_name);
                if frame_addr_func_found.is_some() {
                    frame_addr_func_found.unwrap()
                } else {
                    let llvm_ctx = context.llvm_context();
                    let frame_addr_ret_type = llvm_ctx.i8_type().ptr_type(AddressSpace::Generic);
                    let arg1 = llvm_ctx.i32_type();
                    let frame_addr_func_type = frame_addr_ret_type.fn_type(&[arg1.into()], false);
                    let frame_addr_func = module.add_function(frame_addr_func_name, frame_addr_func_type, Some(External));

                    let attr_factory = context.attributes();
                    frame_addr_func.add_attribute(0, *attr_factory.attr_nounwind());
                    frame_addr_func.add_attribute(0, *attr_factory.attr_readnone());
                    frame_addr_func
                }
            }

            // No type
            LLVMIntrinsic::LongJmp => {
                let module = context.module();

                let longjmp_func_name = self.to_name(arg_type);
                let longjmp_func_found = module.get_function(longjmp_func_name);
                if longjmp_func_found.is_some() {
                    longjmp_func_found.unwrap()
                } else {
                    let llvm_ctx = context.llvm_context();
                    let longjmp_ret_type = llvm_ctx.void_type();
                    let arg1 = llvm_ctx.i8_type().ptr_type(AddressSpace::Generic);
                    let longjmp_func_type = longjmp_ret_type.fn_type(&[arg1.into()], false);
                    let longjmp_func = module.add_function(longjmp_func_name, longjmp_func_type, Some(External));

                    let attr_factory = context.attributes();
                    longjmp_func.add_attribute(0, *attr_factory.attr_nounwind());
                    longjmp_func.add_attribute(0, *attr_factory.attr_noreturn());
                    longjmp_func
                }
            }

            // No type
            LLVMIntrinsic::StackSave => {
                let module = context.module();

                let stacksave_func_name = self.to_name(arg_type);
                let stacksave_func_found = module.get_function(stacksave_func_name);
                if stacksave_func_found.is_some() {
                    stacksave_func_found.unwrap()
                } else {
                    let llvm_ctx = context.llvm_context();
                    let stack_save_ret_type = llvm_ctx.i8_type().ptr_type(AddressSpace::Generic);
                    let stack_save_func_type = stack_save_ret_type.fn_type(&[], false);
                    let stack_save_func = module.add_function(stacksave_func_name, stack_save_func_type, Some(External));

                    let attr_factory = context.attributes();
                    stack_save_func.add_attribute(0, *attr_factory.attr_nounwind());
                    stack_save_func
                }
            }

            // No type
            LLVMIntrinsic::SetJmp => {
                let module = context.module();

                let setjmp_func_name = self.to_name(arg_type);
                let setjmp_func_found = module.get_function(setjmp_func_name);
                if setjmp_func_found.is_some() {
                    setjmp_func_found.unwrap()
                } else {
                    let llvm_ctx = context.llvm_context();
                    let setjmp_ret_type = llvm_ctx.i32_type();
                    let arg1 = llvm_ctx.i8_type().ptr_type(AddressSpace::Generic);
                    let setjmp_func_type = setjmp_ret_type.fn_type(&[arg1.into()], false);
                    let setjmp_func = module.add_function(setjmp_func_name, setjmp_func_type, Some(External));

                    let attr_factory = context.attributes();
                    setjmp_func.add_attribute(0, *attr_factory.attr_nounwind());
                    setjmp_func
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use inkwell::attributes::Attribute;
    use inkwell::context::Context;
    use inkwell::module::Linkage::*;
    use inkwell::types::IntType;

    use super::*;

    #[test]
    fn test_intrinsic_to_name() {
        let context = Context::create();

        let i256_t = IntType::custom_width_int_type(256);
        let i160_t = IntType::custom_width_int_type(160);
        let type_enum_i256_t = BasicTypeEnum::IntType(i256_t);
        let type_enum_i160_t = BasicTypeEnum::IntType(i160_t);
        let type_enum_i32_t = BasicTypeEnum::IntType(context.i32_type());
        let type_enum_i64_t = BasicTypeEnum::IntType(context.i64_type());

        assert_eq!(LLVMIntrinsic::FrameAddress.to_name(None), "llvm.frameaddress");
        assert_eq!(LLVMIntrinsic::LongJmp.to_name(None), "llvm.eh.sjlj.longjmp");
        assert_eq!(LLVMIntrinsic::StackSave.to_name(None), "llvm.stacksave");
        assert_eq!(LLVMIntrinsic::SetJmp.to_name(None), "llvm.eh.sjlj.setjmp");
        assert_eq!(LLVMIntrinsic::Bswap.to_name(Some(type_enum_i256_t)), "llvm.bswap.i256");
        assert_eq!(LLVMIntrinsic::Bswap.to_name(Some(type_enum_i160_t)), "llvm.bswap.i160");
        assert_eq!(LLVMIntrinsic::Ctlz.to_name(Some(type_enum_i256_t)), "llvm.ctlz.i256");

        assert_eq!(
            LLVMIntrinsic::MemSet.to_name(Some(type_enum_i32_t)),
            "llvm.memset.p0i8.i32"
        );
        assert_eq!(
            LLVMIntrinsic::MemSet.to_name(Some(type_enum_i64_t)),
            "llvm.memset.p0i8.i64"
        );
    }

    #[test]
    fn test_intrinsic_bswap256_decl() {
        let jitctx = JITContext::new();
        let types_instance = jitctx.evm_types();
        let word_type = types_instance.get_word_type();
        let enum_word_type = BasicTypeEnum::IntType(word_type);
        let func_decl = LLVMIntrinsic::Bswap.get_intrinsic_declaration(&jitctx, Some(enum_word_type));
        assert_eq!(func_decl.count_params(), 1);
        let func_name = func_decl.get_name();
        assert_eq!(
            func_name.to_str(),
            Ok(LLVMIntrinsic::Bswap.to_name(Some(enum_word_type)))
        );

        let arg1 = func_decl.get_first_param().unwrap();
        assert!(arg1.get_type().is_int_type());
        assert_eq!(arg1.get_type().into_int_type().get_bit_width(), 256);

        let ret_t = word_type;
        assert_eq!(func_decl.get_return_type(), BasicTypeEnum::IntType(ret_t));
        assert!(func_decl.get_linkage() == External);

        let nounwind_attr = func_decl.get_enum_attribute(0, Attribute::get_named_enum_kind_id("nounwind"));
        assert!(nounwind_attr != None);

        let readnone_attr = func_decl.get_enum_attribute(0, Attribute::get_named_enum_kind_id("readnone"));
        assert!(readnone_attr != None);

        let speculatable_attr = func_decl.get_enum_attribute(0, Attribute::get_named_enum_kind_id("speculatable"));
        assert!(speculatable_attr != None);
    }

    #[test]
    fn test_intrinsic_bswap160_decl() {
        let jitctx = JITContext::new();
        let types_instance = jitctx.evm_types();
        let addr_type = types_instance.get_address_type();
        let enum_addr_type = BasicTypeEnum::IntType(addr_type);
        let func_decl = LLVMIntrinsic::Bswap.get_intrinsic_declaration(&jitctx, Some(enum_addr_type));
        assert_eq!(func_decl.count_params(), 1);
        let func_name = func_decl.get_name();
        assert_eq!(
            func_name.to_str(),
            Ok(LLVMIntrinsic::Bswap.to_name(Some(enum_addr_type)))
        );

        let arg1 = func_decl.get_first_param().unwrap();
        assert!(arg1.get_type().is_int_type());
        assert_eq!(arg1.get_type().into_int_type().get_bit_width(), 160);

        let ret_t = addr_type;
        assert_eq!(func_decl.get_return_type(), BasicTypeEnum::IntType(ret_t));
        assert!(func_decl.get_linkage() == External);

        let nounwind_attr = func_decl.get_enum_attribute(0, Attribute::get_named_enum_kind_id("nounwind"));
        assert!(nounwind_attr != None);

        let readnone_attr = func_decl.get_enum_attribute(0, Attribute::get_named_enum_kind_id("readnone"));
        assert!(readnone_attr != None);

        let speculatable_attr = func_decl.get_enum_attribute(0, Attribute::get_named_enum_kind_id("speculatable"));
        assert!(speculatable_attr != None);
    }

    #[test]
    fn test_intrinsic_bswap64_decl() {
        let jitctx = JITContext::new();
        let types_instance = jitctx.evm_types();
        let addr_type = types_instance.get_size_type();
        let enum_addr_type = BasicTypeEnum::IntType(addr_type);
        let func_decl = LLVMIntrinsic::Bswap.get_intrinsic_declaration(&jitctx, Some(enum_addr_type));
        assert_eq!(func_decl.count_params(), 1);
        let func_name = func_decl.get_name();
        assert_eq!(
            func_name.to_str(),
            Ok(LLVMIntrinsic::Bswap.to_name(Some(enum_addr_type)))
        );

        let arg1 = func_decl.get_first_param().unwrap();
        assert!(arg1.get_type().is_int_type());
        assert_eq!(arg1.get_type().into_int_type().get_bit_width(), 64);

        let ret_t = addr_type;
        assert_eq!(func_decl.get_return_type(), BasicTypeEnum::IntType(ret_t));
        assert!(func_decl.get_linkage() == External);

        let nounwind_attr = func_decl.get_enum_attribute(0, Attribute::get_named_enum_kind_id("nounwind"));
        assert!(nounwind_attr != None);

        let readnone_attr = func_decl.get_enum_attribute(0, Attribute::get_named_enum_kind_id("readnone"));
        assert!(readnone_attr != None);

        let speculatable_attr = func_decl.get_enum_attribute(0, Attribute::get_named_enum_kind_id("speculatable"));
        assert!(speculatable_attr != None);
    }

    #[test]
    fn test_intrinsic_ctlz256_decl() {
        let jitctx = JITContext::new();
        let types_instance = jitctx.evm_types();
        let word_type = types_instance.get_word_type();
        let enum_word_type = BasicTypeEnum::IntType(word_type);
        let func_decl = LLVMIntrinsic::Ctlz.get_intrinsic_declaration(&jitctx, Some(enum_word_type));
        assert_eq!(func_decl.count_params(), 2);
        let func_name = func_decl.get_name();
        assert_eq!(
            func_name.to_str(),
            Ok(LLVMIntrinsic::Ctlz.to_name(Some(enum_word_type)))
        );

        let arg1 = func_decl.get_nth_param(0).unwrap();
        assert!(arg1.get_type().is_int_type());
        assert_eq!(arg1.get_type().into_int_type().get_bit_width(), 256);

        let arg2 = func_decl.get_nth_param(1).unwrap();
        assert!(arg2.get_type().is_int_type());
        assert_eq!(arg2.get_type().into_int_type().get_bit_width(), 1);

        let ret_t = word_type;
        assert_eq!(func_decl.get_return_type(), BasicTypeEnum::IntType(ret_t));
        assert!(func_decl.get_linkage() == External);

        let nounwind_attr = func_decl.get_enum_attribute(0, Attribute::get_named_enum_kind_id("nounwind"));
        assert!(nounwind_attr != None);

        let readnone_attr = func_decl.get_enum_attribute(0, Attribute::get_named_enum_kind_id("readnone"));
        assert!(readnone_attr != None);

        let speculatable_attr = func_decl.get_enum_attribute(0, Attribute::get_named_enum_kind_id("speculatable"));
        assert!(speculatable_attr != None);
    }

    #[test]
    fn test_intrinsic_stacksave_decl() {
        let jitctx = JITContext::new();
        let context = jitctx.llvm_context();

        let func_decl = LLVMIntrinsic::StackSave.get_intrinsic_declaration(&jitctx, None);
        assert_eq!(func_decl.count_params(), 0);
        let func_name = func_decl.get_name();
        assert_eq!(func_name.to_str(), Ok(LLVMIntrinsic::StackSave.to_name(None)));

        let ret_t = context.i8_type().ptr_type(AddressSpace::Generic);
        assert_eq!(func_decl.get_return_type(), BasicTypeEnum::PointerType(ret_t));
        assert!(func_decl.get_linkage() == External);
        let nounwind_attr = func_decl.get_enum_attribute(0, Attribute::get_named_enum_kind_id("nounwind"));
        assert!(nounwind_attr != None);
    }

    #[test]
    fn test_intrinsic_frameaddress_decl() {
        let jitctx = JITContext::new();
        let context = jitctx.llvm_context();

        let func_decl = LLVMIntrinsic::FrameAddress.get_intrinsic_declaration(&jitctx, None);
        assert_eq!(func_decl.count_params(), 1);
        let arg1 = func_decl.get_first_param().unwrap();
        assert!(arg1.get_type().is_int_type());
        assert_eq!(arg1.get_type().into_int_type().get_bit_width(), 32);

        let func_name = func_decl.get_name();
        assert_eq!(func_name.to_str(), Ok(LLVMIntrinsic::FrameAddress.to_name(None)));

        let ret_t = context.i8_type().ptr_type(AddressSpace::Generic);
        assert_eq!(func_decl.get_return_type(), BasicTypeEnum::PointerType(ret_t));
        assert!(func_decl.get_linkage() == External);
        let nounwind_attr = func_decl.get_enum_attribute(0, Attribute::get_named_enum_kind_id("nounwind"));
        assert!(nounwind_attr != None);

        let readnone_attr = func_decl.get_enum_attribute(0, Attribute::get_named_enum_kind_id("readnone"));
        assert!(readnone_attr != None);
    }

    #[test]
    fn test_intrinsic_setjmp_decl() {
        let jitctx = JITContext::new();
        let context = jitctx.llvm_context();

        let func_decl = LLVMIntrinsic::SetJmp.get_intrinsic_declaration(&jitctx, None);
        assert_eq!(func_decl.count_params(), 1);
        let arg1 = func_decl.get_first_param().unwrap();
        assert!(arg1.get_type().is_pointer_type());
        let ptr_elt_t = arg1.into_pointer_value().get_type().get_element_type();

        assert!(ptr_elt_t.is_int_type());
        assert_eq!(ptr_elt_t.as_int_type().get_bit_width(), 8);

        let func_name = func_decl.get_name();
        assert_eq!(func_name.to_str(), Ok(LLVMIntrinsic::SetJmp.to_name(None)));

        let ret_t = context.i32_type();
        assert_eq!(func_decl.get_return_type(), BasicTypeEnum::IntType(ret_t));
        assert!(func_decl.get_linkage() == External);
        let nounwind_attr = func_decl.get_enum_attribute(0, Attribute::get_named_enum_kind_id("nounwind"));
        assert!(nounwind_attr != None);
    }

    #[test]
    fn test_intrinsic_longjmp_decl() {
        let jitctx = JITContext::new();

        let func_decl = LLVMIntrinsic::LongJmp.get_intrinsic_declaration(&jitctx, None);
        assert_eq!(func_decl.count_params(), 1);
        let arg1 = func_decl.get_first_param().unwrap();
        assert!(arg1.get_type().is_pointer_type());
        let ptr_elt_t = arg1.into_pointer_value().get_type().get_element_type();

        assert!(ptr_elt_t.is_int_type());
        assert_eq!(ptr_elt_t.as_int_type().get_bit_width(), 8);

        let func_name = func_decl.get_name();
        assert_eq!(func_name.to_str(), Ok(LLVMIntrinsic::LongJmp.to_name(None)));

        assert!(func_decl.get_linkage() == External);
        let nounwind_attr = func_decl.get_enum_attribute(0, Attribute::get_named_enum_kind_id("nounwind"));
        assert!(nounwind_attr != None);

        let noreturn_attr = func_decl.get_enum_attribute(0, Attribute::get_named_enum_kind_id("noreturn"));
        assert!(noreturn_attr != None);
    }

}
