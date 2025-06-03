#![expect(
    clippy::disallowed_types,
    reason = "Using disallowed types without template-default to implement allowed types"
)]
use std::{
    collections::{HashMap, HashSet},
    hash::BuildHasherDefault,
};

/// Helper type to replace [`std::collections::hash_map::RandomState`] is the asset
/// build pipeline - this guarantees deterministic hashing of assets.
pub type DeterministicHasher = siphasher::sip128::SipHasher13;

/// Helper type to replace [`std::hash::BuildHasher`] is the asset build pipeline -
/// this guarantees deterministic hashing of assets.
pub type DeterministicBuildHasher = BuildHasherDefault<DeterministicHasher>;

/// Helper type to replace [`HashMap`] in the asset build pipeline - this guarantees
/// deterministic hashing of assets.
pub type DeterministicHashMap<K, V> = HashMap<K, V, DeterministicBuildHasher>;

/// Helper type to replace [`HashSet`] in the asset build pipeline - this guarantees
/// deterministic hashing of assets.
#[allow(dead_code)]
pub type DeterministicHashSet<V> = HashSet<V, DeterministicBuildHasher>;
