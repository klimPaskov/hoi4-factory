//! HOI4 Factory tool broker (S03): the host's authority check. It makes no network or model
//! calls.
//!
//! * [`grants`]: C-08 approval grants with revocation epochs, checked inside the caller's
//!   transaction immediately before an effect.
//! * [`schema`] and [`registry`]: closed JSON Schemas and the tool registry. Final arguments are
//!   validated after every hook or repair step.
//! * [`keys`]: user API keys, sealed per owner (bring your own key).
//! * [`meter`]: metering that keeps the operator allowance and user-key usage apart, with
//!   budget stops per task and per account.
//! * [`migrator`] applies this crate's migrations (versions 0700 to 0799).
//!
//! Database functions take `&mut PgConnection`: a transaction opened with
//! `factory_accounts::TenantRepo::begin`, so row-level security applies.

mod canonical;
pub mod grants;
mod ids;
pub mod keys;
pub mod meter;
mod migrate;
pub mod registry;
pub mod schema;
mod secret;

pub use canonical::{canonical_json, sha256_hex};
pub use grants::{
    DestinationRefusal, Grant, GrantError, GrantRefusal, GrantSnapshot, NewGrant, OperationClass,
    PathRefusal, authorize_destination, authorize_paths, check_for_effect, get_grant, issue_grant,
    revoke_grant,
};
pub use ids::{GrantId, ReservationId, UserKeyId};
pub use keys::{KeyError, KeyProvider, KeyRecord, KeyStore, NewKey};
pub use meter::{
    BudgetStop, Funding, Meter, MeterError, Reservation, SettleOutcome, Settlement, TaskUsage,
    UsageStatus, UsageSummary, month_start,
};
pub use migrate::{MIGRATION_VERSIONS, migrator};
pub use registry::{
    BrokerRefusal, EffectClass, Hook, HookChange, MAX_ARGUMENT_BYTES, OutputVerdict, PreparedCall,
    RegisteredTool, RegistryError, ToolRegistry,
};
pub use schema::{ClosedSchema, SchemaError, ValidationIssue};
pub use secret::SecretString;
