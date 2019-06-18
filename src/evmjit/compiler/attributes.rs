use inkwell::attributes::Attribute;
use inkwell::context::Context;

#[derive(Debug)]
pub struct LLVMAttributeFactory {
    attr_nounwind: Attribute,
    attr_nocapture: Attribute,
    attr_noalias: Attribute,
    attr_readnone: Attribute,
    attr_noreturn: Attribute,
    attr_speculatable: Attribute,
    attr_argmemonly: Attribute,
    attr_readonly: Attribute,
}

impl LLVMAttributeFactory {
    pub fn new(context: &Context) -> Self {
        let attr_nounwind_id = Attribute::get_named_enum_kind_id("nounwind");
        let attr_nocapture_id = Attribute::get_named_enum_kind_id("nocapture");
        let attr_noalias_id = Attribute::get_named_enum_kind_id("noalias");
        let attr_readnone_id = Attribute::get_named_enum_kind_id("readnone");
        let attr_noreturn_id = Attribute::get_named_enum_kind_id("noreturn");
        let attr_speculatable_id = Attribute::get_named_enum_kind_id("speculatable");
        let attr_argmemonly_id = Attribute::get_named_enum_kind_id("argmemonly");
        let attr_readonly = Attribute::get_named_enum_kind_id("readonly");

        LLVMAttributeFactory {
            attr_nounwind: context.create_enum_attribute(attr_nounwind_id, 0),
            attr_nocapture: context.create_enum_attribute(attr_nocapture_id, 0),
            attr_noalias: context.create_enum_attribute(attr_noalias_id, 0),
            attr_readnone: context.create_enum_attribute(attr_readnone_id, 0),
            attr_noreturn: context.create_enum_attribute(attr_noreturn_id, 0),
            attr_speculatable: context.create_enum_attribute(attr_speculatable_id, 0),
            attr_argmemonly: context.create_enum_attribute(attr_argmemonly_id, 0),
        }
    }
}

impl LLVMAttributeFactory {
    pub fn attr_nounwind(&self) -> &Attribute {
        &self.attr_nounwind
    }

    pub fn attr_nocapture(&self) -> &Attribute {
        &self.attr_nocapture
    }

    pub fn attr_noalias(&self) -> &Attribute {
        &self.attr_noalias
    }

    pub fn attr_readnone(&self) -> &Attribute {
        &self.attr_readnone
    }

    pub fn attr_noreturn(&self) -> &Attribute {
        &self.attr_noreturn
    }

    pub fn attr_speculatable(&self) -> &Attribute {
        &self.attr_speculatable
    }

    pub fn attr_argmemonly(&self) -> &Attribute {
        &self.attr_argmemonly
    }

    pub fn attr_readonly(&self) -> &Attribute {
        &self.attr_readonly
    }
}

#[test]

fn test_llvm_attribute_factory() {
    let context = Context::create();

    let attr_factory = LLVMAttributeFactory::new(&context);
    let nocapture = attr_factory.attr_nocapture();
    let nounwind = attr_factory.attr_nounwind();
    let noalias = attr_factory.attr_noalias();
    let readnone = attr_factory.attr_readnone();
    let noreturn = attr_factory.attr_noreturn();
    let speculatable = attr_factory.attr_speculatable();
    let argmemonly = attr_factory.attr_argmemonly();
    let readonly = attr_factory.attr_readonly();

    assert!(nocapture.is_enum());
    assert_eq!(nocapture.get_enum_value(), 0);
    assert_ne!(nocapture.get_enum_kind_id(), 0);

    assert!(nounwind.is_enum());
    assert_eq!(nounwind.get_enum_value(), 0);
    assert_ne!(nounwind.get_enum_kind_id(), 0);

    assert!(noalias.is_enum());
    assert_eq!(noalias.get_enum_value(), 0);
    assert_ne!(noalias.get_enum_kind_id(), 0);

    assert!(readnone.is_enum());
    assert_eq!(readnone.get_enum_value(), 0);
    assert_ne!(readnone.get_enum_kind_id(), 0);

    assert!(noreturn.is_enum());
    assert_eq!(noreturn.get_enum_value(), 0);
    assert_ne!(noreturn.get_enum_kind_id(), 0);

    assert!(speculatable.is_enum());
    assert_eq!(speculatable.get_enum_value(), 0);
    assert_ne!(speculatable.get_enum_kind_id(), 0);

    assert!(argmemonly.is_enum());
    assert_eq!(argmemonly.get_enum_value(), 0);
    assert_ne!(argmemonly.get_enum_kind_id(), 0);

    assert!(readonly.is_enum());
    assert_eq!(readonly.get_enum_value(), 0);
    assert_ne!(readonly.get_enum_kind_id(), 0);
}
