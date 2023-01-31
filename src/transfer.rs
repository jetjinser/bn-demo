use std::borrow::Cow;

use ethers_core::{
    abi::{
        decode, encode, AbiArrayType, AbiDecode, AbiEncode, AbiError, AbiType, InvalidOutputType,
        ParamType, Token, Tokenizable, TokenizableItem, Tokenize,
    },
    types::{Address, Selector, U256},
};

use crate::copy::EthCall;

#[derive(Debug, Clone)]
pub struct Transfer {
    pub address: Address,
    pub amount: U256,
}

impl EthCall for Transfer {
    fn function_name() -> Cow<'static, str> {
        "transfer".into()
    }
    fn selector() -> Selector {
        [169, 5, 156, 187]
    }
    fn abi_signature() -> Cow<'static, str> {
        "transfer(address,uint256)".into()
    }
}

impl AbiType for Transfer {
    fn param_type() -> ParamType {
        ParamType::Tuple(vec![
            <Address as AbiType>::param_type(),
            <String as AbiType>::param_type(),
            <String as AbiType>::param_type(),
        ])
    }
}

impl AbiArrayType for Transfer {}

impl Tokenizable for Transfer
where
    Address: Tokenize,
    String: Tokenize,
    String: Tokenize,
{
    fn from_token(token: Token) -> Result<Self, InvalidOutputType>
    where
        Self: Sized,
    {
        if let Token::Tuple(tokens) = token {
            if tokens.len() != 2usize {
                return Err(InvalidOutputType({
                    format!("Expected {} tokens, got {}: {:?}", 2, tokens.len(), tokens)
                }));
            }
            let mut iter = tokens.into_iter();
            Ok(Self {
                address: Tokenizable::from_token(iter.next().unwrap())?,
                amount: Tokenizable::from_token(iter.next().unwrap())?,
            })
        } else {
            Err(InvalidOutputType({
                format!("Expected Tuple, got {:?}", token)
            }))
        }
    }
    fn into_token(self) -> Token {
        Token::Tuple(vec![self.address.into_token(), self.amount.into_token()])
    }
}

impl TokenizableItem for Transfer
where
    Address: Tokenize,
    U256: Tokenize,
{
}

impl AbiDecode for Transfer {
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

impl AbiEncode for Transfer {
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
