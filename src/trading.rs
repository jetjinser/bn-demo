mod balance_of;
mod copy;
mod transfer;

use std::{
    env,
    sync::atomic::{AtomicUsize, Ordering},
};

use balance_of::BalanceOf;
use blocknative_flows::{listen_to_address, Event};
use slack_flows::send_message_to_channel;
use transfer::Transfer;

use ethers_core::{
    abi::AbiEncode,
    types::{self, transaction::eip2718::TypedTransaction, Address, TransactionRequest},
};
use ethers_signers::LocalWallet;

use http_req::{
    request::{Method, Request},
    uri::Uri,
};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// XXX: minimum eth
const MIN_ETH: f32 = 30.0;

// XXX: tx wei value
const VALUE: u32 = 1_000_000_000;

// XXX: specify chain_id
const CHAIN_ID: u8 = 5;

// XXX: `from` address
const FROM: &str = "0x1291351b8Aa33FdC64Ac77C8302Db523d5B43AeF";

#[no_mangle]
pub fn run() {
    listen_to_address(FROM, |bnm| {
        match _run(bnm) {
            Ok(msg) => {
                send_message_to_channel("ham-5b68442", "general", format!("success: {}", msg))
            }
            Err(e) => send_message_to_channel("ham-5b68442", "general", format!("faild: {}", e)),
        }
        // if let Ok(msg) = _run(bnm) {
        //     send_message_to_channel("ham-5b68442", "general", format!("success: {}", msg))
        // }
    })
}

// singleton {{{

static COUNTER: AtomicUsize = AtomicUsize::new(1);
fn get_id() -> usize {
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn __local_wallet() -> &'static String {
    static INSTANCE: OnceCell<String> = OnceCell::new();
    INSTANCE.get_or_init(|| env::var("PRIVATE_KEY").expect("no env variable: PRIVATE_KEY"))
}
fn _local_wallet() -> &'static LocalWallet {
    static INSTANCE: OnceCell<LocalWallet> = OnceCell::new();
    INSTANCE.get_or_init(|| {
        let private_key = __local_wallet();
        private_key.parse().unwrap()
    })
}

fn __infura_api() -> &'static String {
    static INSTANCE: OnceCell<String> = OnceCell::new();
    INSTANCE.get_or_init(|| env::var("INFURA_API").expect("no env variable: INFURA_API"))
}

fn _infura_api() -> &'static Uri<'static> {
    static INSTANCE: OnceCell<Uri> = OnceCell::new();
    INSTANCE.get_or_init(|| {
        let infura_api = __infura_api();
        Uri::try_from(infura_api.as_str()).expect("uri error")
    })
}

fn __contract_address() -> &'static String {
    static INSTANCE: OnceCell<String> = OnceCell::new();
    INSTANCE.get_or_init(|| env::var("CONTRACT_ADDRESS").unwrap_or_default())
}

fn _contract_address() -> &'static Address {
    static INSTANCE: OnceCell<Address> = OnceCell::new();
    INSTANCE.get_or_init(|| {
        let contract_address = __contract_address();
        contract_address.parse().unwrap()
    })
}

const MIN_WEI: u128 = (MIN_ETH * 1e18) as u128;

// }}}

// model {{{

#[derive(Deserialize, Serialize)]
struct EthBalance {
    status: String, // 1 if success
    message: String,
    result: String, // wei
}

// }}}

// real main {{{

fn _run(event: Event) -> Result<String, String> {
    let address = event.watched_address;

    let contract_address = __contract_address();
    if contract_address.is_empty() {
        _run_eth(address)
    } else {
        _run_erc20(address)
    }
}

fn _run_eth(address: String) -> Result<String, String> {
    let balance = _get_balance(&address, get_id())?;

    if balance < MIN_WEI {
        _send_tx(address)
            .map(|s| format!("tx sended: {}", s))
            .map_err(|e| format!("_run_eth:\n{}", e))
    } else {
        Ok(String::from("ignored"))
    }
}

fn _run_erc20(address: String) -> Result<String, String> {
    let balance = _get_balance_erc20(&address, get_id())?;

    if balance < MIN_WEI {
        _send_tx_erc20(address)
            .map(|s| format!("tx sended: {}", s))
            .map_err(|e| format!("_run_erc20:\n{}", e))
    } else {
        Ok(String::from("ignored"))
    }
}

// }}}

// common {{{

fn _get_balance(address: &str, id: usize) -> Result<u128, String> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getBalance",
        "params": [address, "latest"],
        "id": id,
    })
    .to_string();

    let value = _make_req(body)?;

    let hex_balance = value
        .get("result")
        .ok_or(format!("[_get_balance] value no result: {:#}", value))?
        .as_str()
        .ok_or(format!("[_get_balance] bottom"))?;

    u128::from_str_radix(hex_balance.trim_start_matches("0x"), 16)
        .map_err(|e| format!("[_get_balance] parse error: {}, raw: {}", e, hex_balance))
}

fn _sign_tx(tx: &TypedTransaction) -> types::Bytes {
    let wallet = _local_wallet();
    let signature = wallet.sign_transaction_sync(&tx);

    tx.rlp_signed(&signature)
}

fn _send_raw_tx(bytes: types::Bytes, id: usize) -> Result<String, String> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_sendRawTransaction",
        "params": [bytes],
        "id": id,
    })
    .to_string();

    let value = _make_req(body)?;

    let result = value["result"]
        .as_str()
        .ok_or(format!("[_send_raw_tx] value no result: {}", value))?;

    Ok(result.to_string())
}

fn _estimate_gas(id: usize, param: Value) -> Result<u64, String> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_estimateGas",
        "params": [param],
        "id": id,
    })
    .to_string();

    let value = _make_req(body)?;

    let hex_gas = value
        .get("result")
        .ok_or(format!("value no result: {:#}", value))?
        .as_str()
        .ok_or(format!("bottom"))?;

    let gas = u64::from_str_radix(hex_gas.trim_start_matches("0x"), 16)
        .map_err(|e| format!("parse error: {}, raw: {}", e, hex_gas))?;

    Ok(gas)
}

pub fn _get_gas_price(id: usize) -> Result<String, String> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_gasPrice",
        "params": [],
        "id": id,
    })
    .to_string();

    let value = _make_req(body)?;

    let hex_gas_price = value
        .get("result")
        .ok_or(format!("value no result: {:#}", value))?
        .as_str()
        .ok_or(format!("bottom"))?;

    Ok(hex_gas_price.to_string())
}

fn _get_tx_count(address: &str, id: usize) -> Result<usize, String> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getTransactionCount",
        "params": [address, "latest"],
        "id": id,
    })
    .to_string();

    let value = _make_req(body)?;

    let hex_count = value
        .get("result")
        .ok_or(format!("value no result: {:#}", value))?
        .as_str()
        .ok_or(format!("bottom"))?;

    let count = usize::from_str_radix(hex_count.trim_start_matches("0x"), 16)
        .map_err(|e| format!("parse error: {}, raw: {}", e, hex_count))?;

    Ok(count)
}

// }}}

// send {{{

fn _send_tx(to: String) -> Result<String, String> {
    let hex_gas_price = _get_gas_price(get_id())?;

    let tx_p = serde_json::json!({
        "to": to,
        "gasPrice": hex_gas_price,
    });

    let gas_price = u128::from_str_radix(hex_gas_price.trim_start_matches("0x"), 16)
        .map_err(|e| format!("parse error: {}, raw: {}", e, hex_gas_price))?;

    let gas = _estimate_gas(get_id(), tx_p)?;

    let nonce = _get_tx_count(FROM, get_id())?;

    let tx: TypedTransaction = TransactionRequest {
        from: Some(FROM.parse::<Address>().unwrap().into()),
        to: Some(to.parse::<Address>().unwrap().into()),
        value: Some(VALUE.into()),
        gas: Some((gas * 2).into()),
        nonce: Some(nonce.into()),
        gas_price: Some((gas_price * 2).into()),
        data: None,
        chain_id: Some(CHAIN_ID.into()),
    }
    .into();

    let bytes = _sign_tx(&tx);
    _send_raw_tx(bytes, get_id())
}

fn _send_tx_erc20(to: String) -> Result<String, String> {
    let address = *_contract_address();

    let from_address: Address = FROM
        .parse()
        .map_err(|e| format!("parse `from` address error: {}", e))?;
    let to_address: Address = to
        .parse()
        .map_err(|e| format!("parse `to` address error: {}", e))?;

    let call = Transfer {
        address: to_address,
        amount: VALUE.into(),
    };

    let data: types::Bytes = call.encode().into();

    let hex_gas_price = _get_gas_price(get_id())?;

    let tx_p = serde_json::json!({
        "to": to,
        "gasPrice": hex_gas_price,
    });

    let gas_price = u128::from_str_radix(hex_gas_price.trim_start_matches("0x"), 16)
        .map_err(|e| format!("parse error: {}, raw: {}", e, hex_gas_price))?;

    let ext = data.len() as u64 * 68 * 10;
    let gas = _estimate_gas(get_id(), tx_p)? + ext;

    let nonce = _get_tx_count(FROM, get_id())?;

    let tx: TypedTransaction = TransactionRequest {
        from: Some(from_address),
        to: Some(address.into()),
        value: Some(0x0.into()),
        gas: Some(gas.into()),
        nonce: Some(nonce.into()),
        gas_price: Some((gas_price * 2).into()),
        data: Some(data),
        chain_id: Some(CHAIN_ID.into()),
    }
    .into();

    let bytes = _sign_tx(&tx);

    _send_raw_tx(bytes, get_id())
}

// }}}

fn _get_balance_erc20(address: &str, id: usize) -> Result<u128, String> {
    let address: Address = address
        .parse()
        .map_err(|e| format!("parse address error: {}", e))?;
    let call = BalanceOf { account: address };

    let data: types::Bytes = call.encode().into();

    let contract_address = __contract_address();

    let tx = serde_json::json!({
        "from": FROM,
        "to": contract_address,
        "data": data,
    });

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_call",
        "params": [tx, "latest"],
        "id": id,
    })
    .to_string();

    let value = _make_req(body)?;

    let hex_balance = value
        .get("result")
        .ok_or(format!("[_get_balance_erc20] value no result: {:#}", value))?
        .as_str()
        .ok_or(format!("[_get_balance_erc20] bottom"))?;

    u128::from_str_radix(hex_balance.trim_start_matches("0x"), 16).map_err(|e| {
        format!(
            "[_get_balance_erc20] parse error: {}, raw: {}",
            e, hex_balance
        )
    })
}

fn _make_req(body: String) -> Result<Value, String> {
    let addr = _infura_api();

    let mut buf = Vec::new();
    let _resp = Request::new(&addr)
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .header("Content-Length", &body.len())
        .body(body.as_bytes())
        .send(&mut buf)
        .map_err(|e| format!("request error: {}", e))?;

    let text = std::str::from_utf8(&buf).map_err(|e| format!("buf read error: {}", e))?;
    serde_json::from_str(text).map_err(|e| format!("deserialize error: {}, raw: {}", e, text))
}
