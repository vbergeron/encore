use crate::error::ExternError;
use crate::ffi::ValueEncode;
use crate::value::Value;
use crate::vm::Vm;

pub trait EncodeArgs {
    type Encoded: AsRef<[Value]>;
    fn encode_args(self, vm: &mut Vm) -> Result<Self::Encoded, ExternError>;
}

impl EncodeArgs for () {
    type Encoded = [Value; 0];
    fn encode_args(self, _vm: &mut Vm) -> Result<Self::Encoded, ExternError> {
        Ok([])
    }
}

// Convenience: a bare encodable value is treated as a single-argument call.
// Disjoint from the tuple impls below because no tuple type implements
// `ValueEncode`.
impl<T: ValueEncode> EncodeArgs for T {
    type Encoded = [Value; 1];
    fn encode_args(self, vm: &mut Vm) -> Result<Self::Encoded, ExternError> {
        Ok([self.encode(vm)?])
    }
}

macro_rules! impl_encode_args_tuple {
    ($n:expr; $($name:ident:$idx:tt),+) => {
        impl<$($name),+> EncodeArgs for ($($name,)+)
        where
            $($name: ValueEncode),+
        {
            type Encoded = [Value; $n];
            fn encode_args(self, vm: &mut Vm) -> Result<Self::Encoded, ExternError> {
                let mut out = [Value::ZERO; $n];
                $(out[$idx] = self.$idx.encode(vm)?;)+
                Ok(out)
            }
        }
    };
}

impl_encode_args_tuple!(1; A:0);
impl_encode_args_tuple!(2; A:0, B:1);
impl_encode_args_tuple!(3; A:0, B:1, C:2);
impl_encode_args_tuple!(4; A:0, B:1, C:2, D:3);
impl_encode_args_tuple!(5; A:0, B:1, C:2, D:3, E:4);
impl_encode_args_tuple!(6; A:0, B:1, C:2, D:3, E:4, F:5);
impl_encode_args_tuple!(7; A:0, B:1, C:2, D:3, E:4, F:5, G:6);
impl_encode_args_tuple!(8; A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7);
