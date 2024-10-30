#![cfg(not(tarpaulin_include))]
use std::{cell::RefCell, rc::Rc};

use cosmwasm_std::Addr;
pub use cw_multi_test;

use cw_multi_test::{App, Module, WasmKeeper};

pub use {
    coin_registry::{MockCoinRegistry, MockCoinRegistryBuilder},
    factory::{MockFactory, MockFactoryBuilder},
    pair::{MockXykPair, MockXykPairBuilder},
    pair_stable::{MockStablePair, MockStablePairBuilder},
    token::{MockToken, MockTokenBuilder},
};

pub mod coin_registry;
pub mod factory;
pub mod pair;
pub mod pair_stable;
pub mod token;
pub mod whitelist;

pub const PALOMADEX: &str = "palomadex";

pub fn palomadex_address() -> Addr {
    Addr::unchecked(PALOMADEX)
}
#[cfg(not(target_arch = "wasm32"))]
pub type WKApp<B, A, S, C, X, D, I, G, T> = Rc<
    RefCell<
        App<B, A, S, C, WasmKeeper<<C as Module>::ExecT, <C as Module>::QueryT>, X, D, I, G, T>,
    >,
>;
