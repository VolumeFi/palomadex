#![cfg(not(tarpaulin_include))]

use anyhow::Result as AnyResult;
use cosmwasm_std::{coins, Addr, Binary};
use cw20::MinterResponse;
use cw_multi_test::{App, AppResponse, ContractWrapper, Executor};

use palomadex::asset::{AssetInfo, PairInfo};
use palomadex::factory::{PairConfig, PairType, QueryMsg};

pub struct FactoryHelper {
    // pub owner: Addr,
    pub factory: Addr,
    pub cw20_token_code_id: u64,
}

impl FactoryHelper {
    pub fn init(router: &mut App, owner: &Addr) -> Self {
        let astro_token_contract = Box::new(ContractWrapper::new_with_empty(
            cw20_base::contract::execute,
            cw20_base::contract::instantiate,
            cw20_base::contract::query,
        ));

        let cw20_token_code_id = router.store_code(astro_token_contract);

        let pair_contract = Box::new(
            ContractWrapper::new_with_empty(
                palomadex_pair::contract::execute,
                palomadex_pair::contract::instantiate,
                palomadex_pair::contract::query,
            )
            .with_reply_empty(palomadex_pair::contract::reply),
        );

        let pair_code_id = router.store_code(pair_contract);

        let factory_contract = Box::new(
            ContractWrapper::new_with_empty(
                palomadex_factory::contract::execute,
                palomadex_factory::contract::instantiate,
                palomadex_factory::contract::query,
            )
            .with_reply_empty(palomadex_factory::contract::reply),
        );

        let factory_code_id = router.store_code(factory_contract);

        let msg = palomadex::factory::InstantiateMsg {
            pair_configs: vec![
                PairConfig {
                    code_id: pair_code_id,
                    pair_type: PairType::Xyk {},
                    total_fee_bps: 0,
                    maker_fee_bps: 0,
                    is_disabled: false,
                    is_generator_disabled: false,
                    permissioned: false,
                },
                PairConfig {
                    code_id: pair_code_id,
                    pair_type: PairType::Stable {},
                    total_fee_bps: 0,
                    maker_fee_bps: 0,
                    is_disabled: false,
                    is_generator_disabled: false,
                    permissioned: false,
                },
            ],
            token_code_id: cw20_token_code_id,
            fee_address: None,
            generator_address: None,
            owner: owner.to_string(),
            whitelist_code_id: 0,
            coin_registry_address: "coin_registry".to_string(),
        };

        let factory = router
            .instantiate_contract(
                factory_code_id,
                owner.clone(),
                &msg,
                &[],
                String::from("ASTRO"),
                None,
            )
            .unwrap();

        Self {
            // owner: owner.clone(),
            factory,
            cw20_token_code_id,
        }
    }

    pub fn create_pair(
        &mut self,
        router: &mut App,
        sender: &Addr,
        pair_type: PairType,
        asset_infos: [AssetInfo; 2],
        init_params: Option<Binary>,
    ) -> AnyResult<Addr> {
        let msg = palomadex::factory::ExecuteMsg::CreatePair {
            pair_type,
            asset_infos: asset_infos.to_vec(),
            init_params,
        };

        router.execute_contract(sender.clone(), self.factory.clone(), &msg, &[])?;

        let res: PairInfo = router.wrap().query_wasm_smart(
            self.factory.clone(),
            &QueryMsg::Pair {
                asset_infos: asset_infos.to_vec(),
            },
        )?;

        Ok(res.contract_addr)
    }
}

pub fn instantiate_token(
    app: &mut App,
    token_code_id: u64,
    owner: &Addr,
    token_name: &str,
    decimals: Option<u8>,
) -> Addr {
    let init_msg = palomadex::token::InstantiateMsg {
        name: token_name.to_string(),
        symbol: token_name.to_string(),
        decimals: decimals.unwrap_or(6),
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: owner.to_string(),
            cap: None,
        }),
        marketing: None,
    };

    app.instantiate_contract(
        token_code_id,
        owner.clone(),
        &init_msg,
        &[],
        token_name,
        None,
    )
    .unwrap()
}

pub fn mint(
    app: &mut App,
    owner: &Addr,
    token: &Addr,
    amount: u128,
    receiver: &Addr,
) -> AnyResult<AppResponse> {
    app.execute_contract(
        owner.clone(),
        token.clone(),
        &cw20::Cw20ExecuteMsg::Mint {
            recipient: receiver.to_string(),
            amount: amount.into(),
        },
        &[],
    )
}

pub fn mint_native(
    app: &mut App,
    denom: &str,
    amount: u128,
    receiver: &Addr,
) -> AnyResult<AppResponse> {
    // .init_balance() erases previous balance thus we use such hack and create intermediate "denom admin"
    let denom_admin = Addr::unchecked(format!("{denom}_admin"));
    let coins_vec = coins(amount, denom);
    app.init_modules(|router, _, storage| {
        router
            .bank
            .init_balance(storage, &denom_admin, coins_vec.clone())
    })
    .unwrap();

    app.send_tokens(denom_admin, receiver.clone(), &coins_vec)
}
