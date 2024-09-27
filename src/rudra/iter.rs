//! A various utility iterators that iterate over Rustc internal items.
//! Many of these internally use `Vec`. Directly returning that `Vec` might be
//! more performant, but we are intentionally trying to hide the implementation
//! detail here.

use crate::rudra::context::CtxOwner;
use charon_lib::types::{TraitDeclId, TraitImplId};

/// Given a trait `DefId`, this iterator returns `HirId` of all local impl blocks
/// that implements that trait.
pub struct LocalTraitIter {
    inner: std::vec::IntoIter<TraitImplId>,
}

impl LocalTraitIter {
    pub fn new(ctx: &CtxOwner, trait_def_id: TraitDeclId) -> Self {
        // We do something
        let impl_id_vec = ctx
            .trait_impl_map
            .get(&trait_def_id)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or(Vec::new());
        LocalTraitIter {
            inner: impl_id_vec.into_iter(),
        }
    }
}

impl Iterator for LocalTraitIter {
    type Item = TraitImplId;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}
