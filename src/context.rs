use charon_lib::types::{TraitDeclId, TraitImplId};
use charon_lib::ullbc_ast::TranslatedCrate;
use std::collections::{HashMap, HashSet};

pub struct Ctx {
    pub crate_data: TranslatedCrate,
    /// The map from trait decl ids to trait impls
    pub trait_impl_map: HashMap<TraitDeclId, HashSet<TraitImplId>>,
}

impl Ctx {
    pub fn new(crate_data: TranslatedCrate) -> Self {
        let mut trait_impl_map = HashMap::new();
        for tdecl in &crate_data.trait_decls {
            trait_impl_map.insert(tdecl.def_id, HashSet::new());
        }
        for timpl in &crate_data.trait_impls {
            match trait_impl_map.get_mut(&timpl.impl_trait.trait_id) {
                None => {
                    trait_impl_map.insert(timpl.impl_trait.trait_id, HashSet::from([timpl.def_id]));
                }
                Some(s) => {
                    let _ = s.insert(timpl.def_id);
                }
            }
        }
        Ctx {
            crate_data,
            trait_impl_map,
        }
    }
}
