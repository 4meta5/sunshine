use grandpa_primitives::AuthorityId as GrandpaId;
use node_template_runtime::{
    AccountId,
    AuraConfig,
    BalancesConfig,
    GenesisConfig,
    GrandpaConfig,
    Share,
    ShareId,
    SharesConfig,
    Signature,
    SudoConfig,
    SystemConfig,
    WASM_BINARY, // Signal, VoteId
};
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::ChainSpec<GenesisConfig>;

/// The chain specification option. This is expected to come in from the CLI and
/// is little more than one of a number of alternatives which can easily be converted
/// from a string (`--chain=...`) into a `ChainSpec`.
#[derive(Clone, Debug)]
pub enum Alternative {
    /// Whatever the current runtime is, with just Alice as an auth.
    Development,
    /// Whatever the current runtime is, with simple Alice/Bob auths.
    LocalTestnet,
}

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

type AccountPublic = <Signature as Verify>::Signer;

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Helper function to generate a ShareId from a u64
pub fn get_share_id_from_u64(value: u64) -> ShareId {
    value.into()
}

/// Helper function to generate a Share from a u64
pub fn get_share_from_u64(value: u64) -> Share {
    value.into()
}

/// Helper function to generate an authority key for Aura
pub fn get_authority_keys_from_seed(s: &str) -> (AuraId, GrandpaId) {
    (get_from_seed::<AuraId>(s), get_from_seed::<GrandpaId>(s))
}

impl Alternative {
    /// Get an actual chain config from one of the alternatives.
    pub(crate) fn load(self) -> Result<ChainSpec, String> {
        Ok(match self {
            Alternative::Development => ChainSpec::from_genesis(
                "Development",
                "dev",
                || {
                    testnet_genesis(
                        // initial authorities
                        vec![get_authority_keys_from_seed("Alice")],
                        // root key
                        get_account_id_from_seed::<sr25519::Public>("Alice"),
                        // endowed accounts
                        vec![
                            get_account_id_from_seed::<sr25519::Public>("Alice"),
                            get_account_id_from_seed::<sr25519::Public>("Bob"),
                            get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
                            get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
                        ],
                        // membership shares
                        vec![
                            (
                                get_account_id_from_seed::<sr25519::Public>("Alice"),
                                get_share_id_from_u64(1),
                                get_share_from_u64(10),
                            ),
                            (
                                get_account_id_from_seed::<sr25519::Public>("Bob"),
                                get_share_id_from_u64(1),
                                get_share_from_u64(10),
                            ),
                        ],
                        // total issuance
                        vec![(1, 20)],
                        // shareholder membership
                        vec![(
                            1,
                            vec![
                                get_account_id_from_seed::<sr25519::Public>("Alice"),
                                get_account_id_from_seed::<sr25519::Public>("Bob"),
                            ],
                        )],
                        true,
                    )
                },
                vec![],
                None,
                None,
                None,
                None,
            ),
            Alternative::LocalTestnet => ChainSpec::from_genesis(
                "Local Testnet",
                "local_testnet",
                || {
                    testnet_genesis(
                        // initial authorities
                        vec![
                            get_authority_keys_from_seed("Alice"),
                            get_authority_keys_from_seed("Bob"),
                        ],
                        // root key
                        get_account_id_from_seed::<sr25519::Public>("Alice"),
                        // endowed accounts
                        vec![
                            get_account_id_from_seed::<sr25519::Public>("Alice"),
                            get_account_id_from_seed::<sr25519::Public>("Bob"),
                            get_account_id_from_seed::<sr25519::Public>("Charlie"),
                            get_account_id_from_seed::<sr25519::Public>("Dave"),
                            get_account_id_from_seed::<sr25519::Public>("Eve"),
                            get_account_id_from_seed::<sr25519::Public>("Ferdie"),
                            get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
                            get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
                            get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
                            get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
                            get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
                            get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
                        ],
                        // membership shares
                        vec![
                            (
                                get_account_id_from_seed::<sr25519::Public>("Alice"),
                                get_share_id_from_u64(1),
                                get_share_from_u64(10),
                            ),
                            (
                                get_account_id_from_seed::<sr25519::Public>("Bob"),
                                get_share_id_from_u64(1),
                                get_share_from_u64(10),
                            ),
                            (
                                get_account_id_from_seed::<sr25519::Public>("Charlie"),
                                get_share_id_from_u64(1),
                                get_share_from_u64(10),
                            ),
                            (
                                get_account_id_from_seed::<sr25519::Public>("Dave"),
                                get_share_id_from_u64(1),
                                get_share_from_u64(10),
                            ),
                            (
                                get_account_id_from_seed::<sr25519::Public>("Eve"),
                                get_share_id_from_u64(1),
                                get_share_from_u64(10),
                            ),
                            (
                                get_account_id_from_seed::<sr25519::Public>("Ferdie"),
                                get_share_id_from_u64(1),
                                get_share_from_u64(10),
                            ),
                        ],
                        // total issuance
                        vec![(1, 60)],
                        // shareholder membership
                        vec![(
                            1,
                            vec![
                                get_account_id_from_seed::<sr25519::Public>("Alice"),
                                get_account_id_from_seed::<sr25519::Public>("Bob"),
                                get_account_id_from_seed::<sr25519::Public>("Charlie"),
                                get_account_id_from_seed::<sr25519::Public>("Dave"),
                                get_account_id_from_seed::<sr25519::Public>("Eve"),
                                get_account_id_from_seed::<sr25519::Public>("Ferdie"),
                            ],
                        )],
                        true,
                    )
                },
                vec![],
                None,
                None,
                None,
                None,
            ),
        })
    }

    pub(crate) fn from(s: &str) -> Option<Self> {
        match s {
            "dev" => Some(Alternative::Development),
            "" | "local" => Some(Alternative::LocalTestnet),
            _ => None,
        }
    }
}

fn testnet_genesis(
    initial_authorities: Vec<(AuraId, GrandpaId)>,
    root_key: AccountId,
    endowed_accounts: Vec<AccountId>,
    membership_shares: Vec<(AccountId, ShareId, Share)>,
    total_issuance: Vec<(ShareId, Share)>,
    shareholder_membership: Vec<(ShareId, Vec<AccountId>)>,
    _enable_println: bool,
) -> GenesisConfig {
    GenesisConfig {
        system: Some(SystemConfig {
            code: WASM_BINARY.to_vec(),
            changes_trie_config: Default::default(),
        }),
        balances: Some(BalancesConfig {
            balances: endowed_accounts
                .iter()
                .cloned()
                .map(|k| (k, 1 << 60))
                .collect(),
        }),
        shares: Some(SharesConfig {
            membership_shares,
            total_issuance,
            shareholder_membership,
        }),
        aura: Some(AuraConfig {
            authorities: initial_authorities.iter().map(|x| (x.0.clone())).collect(),
        }),
        grandpa: Some(GrandpaConfig {
            authorities: initial_authorities
                .iter()
                .map(|x| (x.1.clone(), 1))
                .collect(),
        }),
        sudo: Some(SudoConfig { key: root_key }),
    }
}

pub fn load_spec(id: &str) -> Result<Option<ChainSpec>, String> {
    Ok(match Alternative::from(id) {
        Some(spec) => Some(spec.load()?),
        None => None,
    })
}
