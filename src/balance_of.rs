use std::borrow::Cow;

use ethers_core::{
    abi::{
        decode, encode, AbiArrayType, AbiDecode, AbiEncode, AbiError, AbiType, InvalidOutputType,
        ParamType, Token, Tokenizable, TokenizableItem, Tokenize,
    },
    types::{Address, Selector},
};

use crate::copy::EthCall;

#[derive(Debug, Clone)]
pub struct BalanceOf {
    pub account: Address,
}

impl EthCall for BalanceOf {
    fn function_name() -> Cow<'static, str> {
        "balanceOf".into()
    }
    fn selector() -> Selector {
        [112, 160, 130, 49]
    }
    fn abi_signature() -> Cow<'static, str> {
        "balanceOf(address)".into()
    }
}

impl AbiType for BalanceOf {
    fn param_type() -> ParamType {
        ParamType::Tuple(vec![
            <Address as AbiType>::param_type(),
            <String as AbiType>::param_type(),
            <String as AbiType>::param_type(),
        ])
    }
}

impl AbiArrayType for BalanceOf {}

impl Tokenizable for BalanceOf
where
    Address: Tokenize,
{
    fn from_token(token: Token) -> Result<Self, InvalidOutputType>
    where
        Self: Sized,
    {
        if let Token::Tuple(tokens) = token {
            if tokens.len() != 1usize {
                return Err(InvalidOutputType({
                    format!("Expected {} tokens, got {}: {:?}", 1, tokens.len(), tokens)
                }));
            }
            let mut iter = tokens.into_iter();
            Ok(Self {
                account: Tokenizable::from_token(iter.next().unwrap())?,
            })
        } else {
            Err(InvalidOutputType({
                format!("Expected Tuple, got {:?}", token)
            }))
        }
    }
    fn into_token(self) -> Token {
        Token::Tuple(vec![self.account.into_token()])
    }
}

impl TokenizableItem for BalanceOf
where
    Address: Tokenize,
    String: Tokenize,
    String: Tokenize,
{
}

impl AbiDecode for BalanceOf {
    fn decode(bytes: impl AsRef<[u8]>) -> Result<Self, AbiError> {
        let bytes = bytes.as_ref();
        if bytes.len() < 4 || bytes[..4] != <Self as EthCall>::selector() {
            return Err(AbiError::WrongSelector);
        }
        let data_types = [ParamType::Address];
        let data_tokens = decode(&data_types, &bytes[4..])?;
        Ok(<Self as Tokenizable>::from_token(Token::Tuple(
            data_tokens,
        ))?)
    }
}

impl AbiEncode for BalanceOf {
    fn encode(self) -> Vec<u8> {
        let tokens = Tokenize::into_tokens(self);
        let selector = <Self as EthCall>::selector();
        let encoded = encode(&tokens);
        selector
            .iter()
            .copied()
            .chain(encoded.into_iter())
            .collect()
    }
}
