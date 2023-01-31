use std::borrow::Cow;

use ethers_core::{
    abi::{AbiDecode, AbiEncode, Tokenizable},
    types::Selector,
    utils::id,
};

pub trait EthCall: Tokenizable + AbiDecode + AbiEncode + Send + Sync {
    fn function_name() -> Cow<'static, str>;

    fn abi_signature() -> Cow<'static, str>;

    fn selector() -> Selector {
        id(Self::abi_signature())
    }
}
