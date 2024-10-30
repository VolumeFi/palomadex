use cw_storage_plus::Item;
use palomadex::common::OwnershipProposal;
use palomadex::native_coin_registry::Config;

/// Stores the contract config at the given key
pub const CONFIG: Item<Config> = Item::new("config");

/// Contains a proposal to change contract ownership.
pub const OWNERSHIP_PROPOSAL: Item<OwnershipProposal> = Item::new("ownership_proposal");
