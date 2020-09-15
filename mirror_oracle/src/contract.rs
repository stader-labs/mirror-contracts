use cosmwasm_std::{
    log, to_binary, Api, Binary, Decimal, Env, Extern, HandleResponse, HandleResult, HumanAddr,
    InitResponse, Querier, StdError, StdResult, Storage,
};

use crate::msg::{AssetResponse, ConfigResponse, HandleMsg, InitMsg, PriceResponse, QueryMsg};

use crate::state::{
    read_asset, read_config, read_price, store_asset, store_config, store_price, Asset, Config,
    Price,
};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    store_config(
        &mut deps.storage,
        &Config {
            owner: deps.api.canonical_address(&msg.owner)?,
            base_denom: msg.base_denom.to_string(),
        },
    )?;

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> HandleResult {
    match msg {
        HandleMsg::UpdateConfig { owner } => try_update_config(deps, env, owner),
        HandleMsg::RegisterAsset {
            symbol,
            feeder,
            token,
        } => try_register_asset(deps, env, symbol, feeder, token),
        HandleMsg::FeedPrice {
            symbol,
            price,
            price_multiplier,
        } => try_feed_price(deps, env, symbol, price, price_multiplier),
    }
}

pub fn try_update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: Option<HumanAddr>,
) -> HandleResult {
    let mut config: Config = read_config(&deps.storage)?;
    if deps.api.canonical_address(&env.message.sender)? != config.owner {
        return Err(StdError::unauthorized());
    }

    if let Some(owner) = owner {
        config.owner = deps.api.canonical_address(&owner)?;
    }

    store_config(&mut deps.storage, &config)?;
    Ok(HandleResponse::default())
}

pub fn try_register_asset<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    symbol: String,
    feeder: HumanAddr,
    token: HumanAddr,
) -> HandleResult {
    if read_asset(&deps.storage, symbol.to_string()).is_ok() {
        return Err(StdError::unauthorized());
    }

    store_asset(
        &mut deps.storage,
        symbol.to_string(),
        &Asset {
            symbol: symbol.to_string(),
            feeder: deps.api.canonical_address(&feeder)?,
            token: deps.api.canonical_address(&token)?,
        },
    )?;

    store_price(
        &mut deps.storage,
        symbol,
        &Price {
            price: Decimal::zero(),
            price_multiplier: Decimal::one(),
            last_update_time: 0u64,
        },
    )?;

    Ok(HandleResponse::default())
}

pub fn try_feed_price<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    symbol: String,
    price: Decimal,
    price_multiplier: Option<Decimal>,
) -> HandleResult {
    let asset: Asset = read_asset(&deps.storage, symbol.to_string())?;
    if deps.api.canonical_address(&env.message.sender)? != asset.feeder {
        return Err(StdError::unauthorized());
    }

    let mut state: Price = read_price(&deps.storage, symbol.to_string())?;
    state.last_update_time = env.block.time;
    state.price = price;
    if let Some(price_multiplier) = price_multiplier {
        state.price_multiplier = price_multiplier;
    }

    store_price(&mut deps.storage, symbol, &state)?;
    let res = HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "price_feed"),
            log("price", &price.to_string()),
        ],
        data: None,
    };

    Ok(res)
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::Asset { symbol } => to_binary(&query_asset(deps, symbol)?),
        QueryMsg::Price { symbol } => to_binary(&query_price(deps, symbol)?),
    }
}

fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let state = read_config(&deps.storage)?;
    let resp = ConfigResponse {
        owner: deps.api.human_address(&state.owner)?,
        base_denom: state.base_denom.to_string(),
    };

    Ok(resp)
}

fn query_asset<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    symbol: String,
) -> StdResult<AssetResponse> {
    let state = read_asset(&deps.storage, symbol)?;
    let resp = AssetResponse {
        symbol: state.symbol,
        feeder: deps.api.human_address(&state.feeder)?,
        token: deps.api.human_address(&state.token)?,
    };

    Ok(resp)
}

fn query_price<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    symbol: String,
) -> StdResult<PriceResponse> {
    let state = read_price(&deps.storage, symbol)?;
    let resp = PriceResponse {
        price: state.price,
        price_multiplier: state.price_multiplier,
        last_update_time: state.last_update_time,
    };

    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cosmwasm_std::StdError;
    use std::str::FromStr;

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            owner: HumanAddr("owner0000".to_string()),
            base_denom: "base0000".to_string(),
        };

        let env = mock_env("addr0000", &[]);

        // we can just call .unwrap() to assert this was a success
        let res = init(&mut deps, env, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let value = query_config(&deps).unwrap();
        assert_eq!("owner0000", value.owner.as_str());
        assert_eq!("base0000", value.base_denom.as_str());
    }

    #[test]
    fn update_config() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            owner: HumanAddr("owner0000".to_string()),
            base_denom: "base0000".to_string(),
        };

        let env = mock_env("addr0000", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        // update owner
        let env = mock_env("owner0000", &[]);
        let msg = HandleMsg::UpdateConfig {
            owner: Some(HumanAddr("owner0001".to_string())),
        };

        let res = handle(&mut deps, env, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let value = query_config(&deps).unwrap();
        assert_eq!("owner0001", value.owner.as_str());
        assert_eq!("base0000", value.base_denom.as_str());

        // Unauthorzied err
        let env = mock_env("owner0000", &[]);
        let msg = HandleMsg::UpdateConfig { owner: None };

        let res = handle(&mut deps, env, msg);
        match res {
            Err(StdError::Unauthorized { .. }) => {}
            _ => panic!("Must return unauthorized error"),
        }
    }

    #[test]
    fn feed_price() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg {
            owner: HumanAddr("owner0000".to_string()),
            base_denom: "base0000".to_string(),
        };

        let env = mock_env("addr0000", &[]);
        let _res = init(&mut deps, env, msg).unwrap();

        // update price
        let env = mock_env("addr0000", &[]);
        let msg = HandleMsg::FeedPrice {
            symbol: "uusd".to_string(),
            price: Decimal::from_str("1.2").unwrap(),
            price_multiplier: None,
        };

        let res = handle(&mut deps, env, msg).unwrap_err();
        match res {
            StdError::GenericErr { msg, .. } => assert_eq!(msg, "no asset data stored"),
            _ => panic!("DO NOT ENTER HERE"),
        }

        let msg = HandleMsg::RegisterAsset {
            symbol: "mAPPL".to_string(),
            feeder: HumanAddr::from("addr0000"),
            token: HumanAddr::from("asset0000"),
        };

        let env = mock_env("addr0000", &[]);
        let _res = handle(&mut deps, env, msg).unwrap();

        let value: AssetResponse = query_asset(&deps, "mAPPL".to_string()).unwrap();
        assert_eq!(
            value,
            AssetResponse {
                symbol: "mAPPL".to_string(),
                feeder: HumanAddr::from("addr0000"),
                token: HumanAddr::from("asset0000"),
            }
        );

        let value: PriceResponse = query_price(&deps, "mAPPL".to_string()).unwrap();
        assert_eq!(
            value,
            PriceResponse {
                price: Decimal::zero(),
                price_multiplier: Decimal::one(),
                last_update_time: 0u64,
            }
        );

        let msg = HandleMsg::FeedPrice {
            symbol: "mAPPL".to_string(),
            price: Decimal::from_str("1.2").unwrap(),
            price_multiplier: None,
        };
        let env = mock_env("addr0000", &[]);
        let _res = handle(&mut deps, env.clone(), msg).unwrap();
        let value: PriceResponse = query_price(&deps, "mAPPL".to_string()).unwrap();
        assert_eq!(
            value,
            PriceResponse {
                price: Decimal::from_str("1.2").unwrap(),
                price_multiplier: Decimal::one(),
                last_update_time: env.block.time,
            }
        );

        // Unautorized try
        let env = mock_env("addr0001", &[]);
        let msg = HandleMsg::FeedPrice {
            symbol: "mAPPL".to_string(),
            price: Decimal::from_str("1.2").unwrap(),
            price_multiplier: None,
        };

        let res = handle(&mut deps, env, msg);
        match res {
            Err(StdError::Unauthorized { .. }) => {}
            _ => panic!("Must return unauthorized error"),
        }
    }
}
