use std::collections::HashSet;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Order, Reply, ReplyOn, Response,
    StdError, StdResult, SubMsg, SubMsgResponse, SubMsgResult, WasmMsg,
};
use cw2::set_contract_version;
use cw_utils::parse_instantiate_response_data;
use itertools::Itertools;

use palomadex::asset::{addr_opt_validate, AssetInfo, PairInfo};
use palomadex::common::{claim_ownership, drop_ownership_proposal, propose_new_owner};
use palomadex::factory::{
    Config, ConfigResponse, ExecuteMsg, FeeInfoResponse, InstantiateMsg, PairConfig, PairType,
    PairsResponse, QueryMsg,
};
use palomadex::pair::InstantiateMsg as PairInstantiateMsg;

use crate::error::ContractError;
use crate::querier::query_pair_info;
use crate::state::{
    check_asset_infos, pair_key, read_pairs, TmpPairInfo, CONFIG, OWNERSHIP_PROPOSAL, PAIRS,
    PAIR_CONFIGS, TMP_PAIR_INFO,
};

/// A `reply` call code ID used in a sub-message.
const INSTANTIATE_PAIR_REPLY_ID: u64 = 1;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:palomadex-factory";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Creates a new contract with the specified parameters packed in the `msg` variable.
///
/// * **msg**  is message which contains the parameters used for creating the contract.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let mut config = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        token_code_id: msg.token_code_id,
        fee_address: None,
        generator_address: None,
        whitelist_code_id: msg.whitelist_code_id,
        coin_registry_address: deps.api.addr_validate(&msg.coin_registry_address)?,
        bonding_curve_factory_address: None,
        pusd_denom: None,
    };

    config.generator_address = addr_opt_validate(deps.api, &msg.generator_address)?;

    config.fee_address = addr_opt_validate(deps.api, &msg.fee_address)?;

    let config_set: HashSet<String> = msg
        .pair_configs
        .iter()
        .map(|pc| pc.pair_type.to_string())
        .collect();

    if config_set.len() != msg.pair_configs.len() {
        return Err(ContractError::PairConfigDuplicate {});
    }

    for pc in msg.pair_configs.iter() {
        // Validate total and maker fee bps
        if !pc.valid_fee_bps() {
            return Err(ContractError::PairConfigInvalidFeeBps {});
        }
        PAIR_CONFIGS.save(deps.storage, pc.pair_type.to_string(), pc)?;
    }
    CONFIG.save(deps.storage, &config)?;

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::new())
}

/// Data structure used to update general contract parameters.
pub struct UpdateConfig {
    /// This is the CW20 token contract code identifier
    token_code_id: Option<u64>,
    /// Contract address to send governance fees to (the Maker)
    fee_address: Option<String>,
    /// Generator contract address
    generator_address: Option<String>,
    /// CW1 whitelist contract code id used to store 3rd party staking rewards
    whitelist_code_id: Option<u64>,
    coin_registry_address: Option<String>,
    bonding_curve_factory_address: Option<String>,
    pusd_denom: Option<String>,
}

/// Exposes all the execute functions available in the contract.
/// * **msg** is an object of type [`ExecuteMsg`].
///
/// ## Variants
/// * **ExecuteMsg::UpdateConfig {
///             token_code_id,
///             fee_address,
///             generator_address,
///         }** Updates general contract parameters.
///
/// * **ExecuteMsg::UpdatePairConfig { config }** Updates a pair type
/// * configuration or creates a new pair type if a [`Custom`] name is used (which hasn't been used before).
///
/// * **ExecuteMsg::CreatePair {
///             pair_type,
///             asset_infos,
///             init_params,
///         }** Creates a new pair with the specified input parameters.
///
/// * **ExecuteMsg::Deregister { asset_infos }** Removes an existing pair from the factory.
/// * The asset information is for the assets that are traded in the pair.
///
/// * **ExecuteMsg::ProposeNewOwner { owner, expires_in }** Creates a request to change contract ownership.
///
/// * **ExecuteMsg::DropOwnershipProposal {}** Removes a request to change contract ownership.
///
/// * **ExecuteMsg::ClaimOwnership {}** Claims contract ownership.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig {
            token_code_id,
            fee_address,
            generator_address,
            whitelist_code_id,
            coin_registry_address,
            bonding_curve_factory_address,
            pusd_denom,
        } => execute_update_config(
            deps,
            info,
            UpdateConfig {
                token_code_id,
                fee_address,
                generator_address,
                whitelist_code_id,
                coin_registry_address,
                bonding_curve_factory_address,
                pusd_denom,
            },
        ),
        ExecuteMsg::UpdatePairConfig { config } => execute_update_pair_config(deps, info, config),
        ExecuteMsg::CreatePair {
            pair_type,
            asset_infos,
            init_params,
        } => execute_create_pair(deps, info, env, pair_type, asset_infos, init_params),
        ExecuteMsg::Deregister { asset_infos } => deregister(deps, info, asset_infos),
        ExecuteMsg::ProposeNewOwner { owner, expires_in } => {
            let config = CONFIG.load(deps.storage)?;

            propose_new_owner(
                deps,
                info,
                env,
                owner,
                expires_in,
                config.owner,
                OWNERSHIP_PROPOSAL,
            )
            .map_err(Into::into)
        }
        ExecuteMsg::DropOwnershipProposal {} => {
            let config = CONFIG.load(deps.storage)?;

            drop_ownership_proposal(deps, info, config.owner, OWNERSHIP_PROPOSAL)
                .map_err(Into::into)
        }
        ExecuteMsg::ClaimOwnership {} => {
            claim_ownership(deps, info, env, OWNERSHIP_PROPOSAL, |deps, new_owner| {
                CONFIG
                    .update::<_, StdError>(deps.storage, |mut v| {
                        v.owner = new_owner;
                        Ok(v)
                    })
                    .map(|_| ())
            })
            .map_err(Into::into)
        }
    }
}

/// Updates general contract settings.
///
/// * **param** is an object of type [`UpdateConfig`] that contains the parameters to update.
///
/// ## Executor
/// Only the owner can execute this.
pub fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    param: UpdateConfig,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    // Permission check
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(fee_address) = param.fee_address {
        // Validate address format
        config.fee_address = Some(deps.api.addr_validate(&fee_address)?);
    }

    if let Some(generator_address) = param.generator_address {
        // Validate the address format
        config.generator_address = Some(deps.api.addr_validate(&generator_address)?);
    }

    if let Some(token_code_id) = param.token_code_id {
        config.token_code_id = token_code_id;
    }

    if let Some(code_id) = param.whitelist_code_id {
        config.whitelist_code_id = code_id;
    }

    if let Some(coin_registry_address) = param.coin_registry_address {
        config.coin_registry_address = deps.api.addr_validate(&coin_registry_address)?;
    }

    if let Some(bonding_curve_factory_address) = param.bonding_curve_factory_address {
        config.bonding_curve_factory_address =
            Some(deps.api.addr_validate(&bonding_curve_factory_address)?);
    }

    if let Some(pusd_denom) = param.pusd_denom {
        config.pusd_denom = Some(pusd_denom);
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

/// Updates a pair type's configuration.
///
/// * **pair_config** is an object of type [`PairConfig`] that contains the pair type information to update.
///
/// ## Executor
/// Only the owner can execute this.
pub fn execute_update_pair_config(
    deps: DepsMut,
    info: MessageInfo,
    pair_config: PairConfig,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Permission check
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Validate total and maker fee bps
    if !pair_config.valid_fee_bps() {
        return Err(ContractError::PairConfigInvalidFeeBps {});
    }

    PAIR_CONFIGS.save(
        deps.storage,
        pair_config.pair_type.to_string(),
        &pair_config,
    )?;

    Ok(Response::new().add_attribute("action", "update_pair_config"))
}

/// Creates a new pair of `pair_type` with the assets specified in `asset_infos`.
///
/// * **pair_type** is the pair type of the newly created pair.
///
/// * **asset_infos** is a vector with assets for which we create a pair.
///
/// * **init_params** These are packed params used for custom pair types that need extra data to be instantiated.
pub fn execute_create_pair(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    pair_type: PairType,
    asset_infos: Vec<AssetInfo>,
    init_params: Option<Binary>,
) -> Result<Response, ContractError> {
    check_asset_infos(deps.api, &asset_infos)?;

    let config = CONFIG.load(deps.storage)?;
    let mut auth_flag: bool = true;
    if let Some(bonding_curve_factory_address) = config.bonding_curve_factory_address {
        if !asset_infos.iter().all(|asset_info| {
            if let AssetInfo::NativeToken { denom } = asset_info {
                if info.sender != bonding_curve_factory_address
                    && denom.contains(bonding_curve_factory_address.to_string().as_str())
                {
                    if auth_flag {
                        auth_flag = false;
                        return true;
                    } else {
                        return false;
                    }
                }
                if let Some(pusd_denom) = &config.pusd_denom {
                    if denom.contains(pusd_denom) {
                        if auth_flag {
                            auth_flag = false;
                            return true;
                        } else {
                            return false;
                        }
                    }
                }
            }
            true
        }) {
            return Err(ContractError::Unauthorized {});
        }
    }

    if PAIRS.has(deps.storage, &pair_key(&asset_infos)) {
        return Err(ContractError::PairWasCreated {});
    }

    // Get pair type from config
    let pair_config = PAIR_CONFIGS
        .load(deps.storage, pair_type.to_string())
        .map_err(|_| ContractError::PairConfigNotFound {})?;

    if pair_config.permissioned && info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Check if pair config is disabled
    if pair_config.is_disabled {
        return Err(ContractError::PairConfigDisabled {});
    }

    let pair_key = pair_key(&asset_infos);
    TMP_PAIR_INFO.save(deps.storage, &TmpPairInfo { pair_key })?;

    let sub_msg: Vec<SubMsg> = vec![SubMsg {
        id: INSTANTIATE_PAIR_REPLY_ID,
        msg: WasmMsg::Instantiate {
            admin: Some(config.owner.to_string()),
            code_id: pair_config.code_id,
            msg: to_json_binary(&PairInstantiateMsg {
                asset_infos: asset_infos.clone(),
                token_code_id: config.token_code_id,
                factory_addr: env.contract.address.to_string(),
                init_params,
            })?,
            funds: vec![],
            label: "Palomadex pair".to_string(),
        }
        .into(),
        gas_limit: None,
        reply_on: ReplyOn::Success,
    }];

    Ok(Response::new()
        .add_submessages(sub_msg)
        .add_attributes(vec![
            attr("action", "create_pair"),
            attr("pair", asset_infos.iter().join("-")),
        ]))
}

/// The entry point to the contract for processing replies from submessages.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg {
        Reply {
            id: INSTANTIATE_PAIR_REPLY_ID,
            result:
                SubMsgResult::Ok(SubMsgResponse {
                    data: Some(data), ..
                }),
        } => {
            let tmp = TMP_PAIR_INFO.load(deps.storage)?;
            if PAIRS.has(deps.storage, &tmp.pair_key) {
                return Err(ContractError::PairWasRegistered {});
            }

            let init_response = parse_instantiate_response_data(data.as_slice())
                .map_err(|e| StdError::generic_err(format!("{e}")))?;

            let pair_contract = deps.api.addr_validate(&init_response.contract_address)?;

            PAIRS.save(deps.storage, &tmp.pair_key, &pair_contract)?;

            Ok(Response::new().add_attributes(vec![
                attr("action", "register"),
                attr("pair_contract_addr", pair_contract),
            ]))
        }
        _ => Err(ContractError::FailedToParseReply {}),
    }
}

/// Removes an existing pair from the factory.
///
/// * **asset_infos** is a vector with assets for which we deregister the pair.
///
/// ## Executor
/// Only the owner can execute this.
pub fn deregister(
    deps: DepsMut,
    info: MessageInfo,
    asset_infos: Vec<AssetInfo>,
) -> Result<Response, ContractError> {
    check_asset_infos(deps.api, &asset_infos)?;

    let config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let pair_addr = PAIRS.load(deps.storage, &pair_key(&asset_infos))?;
    PAIRS.remove(deps.storage, &pair_key(&asset_infos));

    Ok(Response::new().add_attributes(vec![
        attr("action", "deregister"),
        attr("pair_contract_addr", pair_addr),
    ]))
}

/// Exposes all the queries available in the contract.
///
/// ## Queries
/// * **QueryMsg::Config {}** Returns general contract parameters using a custom [`ConfigResponse`] structure.
///
/// * **QueryMsg::Pair { asset_infos }** Returns a [`PairInfo`] object with information about a specific Palomadex pair.
///
/// * **QueryMsg::Pairs { start_after, limit }** Returns an array that contains items of type [`PairInfo`].
///   This returns information about multiple Palomadex pairs
///
/// * **QueryMsg::FeeInfo { pair_type }** Returns the fee structure (total and maker fees) for a specific pair type.
///
/// * **QueryMsg::BlacklistedPairTypes {}** Returns a vector that contains blacklisted pair types (pair types that cannot get ASTRO emissions).
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
        QueryMsg::Pair { asset_infos } => to_json_binary(&query_pair(deps, asset_infos)?),
        QueryMsg::Pairs { start_after, limit } => {
            to_json_binary(&query_pairs(deps, start_after, limit)?)
        }
        QueryMsg::FeeInfo { pair_type } => to_json_binary(&query_fee_info(deps, pair_type)?),
        QueryMsg::BlacklistedPairTypes {} => to_json_binary(&query_blacklisted_pair_types(deps)?),
    }
}

/// Returns a vector that contains blacklisted pair types
pub fn query_blacklisted_pair_types(deps: Deps) -> StdResult<Vec<PairType>> {
    PAIR_CONFIGS
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|result| match result {
            Ok(v) => {
                if v.1.is_disabled || v.1.is_generator_disabled {
                    Some(Ok(v.1.pair_type))
                } else {
                    None
                }
            }
            Err(e) => Some(Err(e)),
        })
        .collect()
}

/// Returns general contract parameters using a custom [`ConfigResponse`] structure.
pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    let resp = ConfigResponse {
        owner: config.owner,
        token_code_id: config.token_code_id,
        pair_configs: PAIR_CONFIGS
            .range(deps.storage, None, None, Order::Ascending)
            .map(|item| Ok(item?.1))
            .collect::<StdResult<Vec<_>>>()?,
        fee_address: config.fee_address,
        generator_address: config.generator_address,
        whitelist_code_id: config.whitelist_code_id,
        coin_registry_address: config.coin_registry_address,
        bonding_curve_factory_address: config.bonding_curve_factory_address,
        pusd_denom: config.pusd_denom,
    };

    Ok(resp)
}

/// Returns a pair's data using the assets in `asset_infos` as input (those being the assets that are traded in the pair).
/// * **asset_infos** is a vector with assets traded in the pair.
pub fn query_pair(deps: Deps, asset_infos: Vec<AssetInfo>) -> StdResult<PairInfo> {
    let pair_addr = PAIRS.load(deps.storage, &pair_key(&asset_infos))?;
    query_pair_info(&deps.querier, pair_addr)
}

/// Returns a vector with pair data that contains items of type [`PairInfo`]. Querying starts at `start_after` and returns `limit` pairs.
/// * **start_after** is a field which accepts a vector with items of type [`AssetInfo`].
///   This is the pair from which we start a query.
///
/// * **limit** sets the number of pairs to be retrieved.
pub fn query_pairs(
    deps: Deps,
    start_after: Option<Vec<AssetInfo>>,
    limit: Option<u32>,
) -> StdResult<PairsResponse> {
    let pairs = read_pairs(deps, start_after, limit)?
        .iter()
        .map(|pair_addr| query_pair_info(&deps.querier, pair_addr))
        .collect::<StdResult<Vec<_>>>()?;

    Ok(PairsResponse { pairs })
}

/// Returns the fee setup for a specific pair type using a [`FeeInfoResponse`] struct.
/// * **pair_type** is a struct that represents the fee information (total and maker fees) for a specific pair type.
pub fn query_fee_info(deps: Deps, pair_type: PairType) -> StdResult<FeeInfoResponse> {
    let config = CONFIG.load(deps.storage)?;
    let pair_config = PAIR_CONFIGS.load(deps.storage, pair_type.to_string())?;

    Ok(FeeInfoResponse {
        fee_address: config.fee_address,
        total_fee_bps: pair_config.total_fee_bps,
        maker_fee_bps: pair_config.maker_fee_bps,
    })
}
