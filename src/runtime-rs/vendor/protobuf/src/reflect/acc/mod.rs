#![doc(hidden)]

use crate::reflect::acc::v1::FieldAccessorFunctions;
use crate::reflect::acc::v1::FieldAccessorImpl;
use crate::reflect::acc::v1::FieldAccessorTrait;
use crate::Message;

pub(crate) mod v1;

pub(crate) enum Accessor {
    V1(Box<dyn FieldAccessorTrait + 'static>),
}

/// Accessor object is constructed in generated code.
/// Should not be used directly.
pub struct FieldAccessor {
    pub(crate) name: &'static str,
    pub(crate) accessor: Accessor,
}

impl FieldAccessor {
    pub(crate) fn new_v1<M: Message>(
        name: &'static str,
        fns: FieldAccessorFunctions<M>,
    ) -> FieldAccessor {
        FieldAccessor {
            name,
            accessor: Accessor::V1(Box::new(FieldAccessorImpl { fns })),
        }
    }
}
