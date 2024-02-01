pub mod policy;
mod transmission;

use self::policy::{Condition, PolicyMatch};
use crate::config::policy::DeletePolicy;
use crate::config::transmission::Transmission;
use rhai::{module_resolvers::FileModuleResolver, Array};
use rhai::{CustomType, TypeBuilder};
use rhai::{Dynamic, Engine, EvalAltResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub fn configure(file: &Path) -> Result<Vec<Instance>, Box<EvalAltResult>> {
    let mut engine = Engine::new();
    let resolver = FileModuleResolver::new_with_path(file.parent().unwrap_or(&PathBuf::from(".")));
    engine.set_module_resolver(resolver);
    engine
        // A transmission API endpoint:
        .build_type::<Transmission>()
        // Instances:
        .build_type::<Instance>()
        // Policies
        .build_type::<PolicyMatch>()
        .build_type::<DeletePolicy>()
        // Conditions
        .build_type::<Condition>();

    Dynamic::from(
        engine
            .eval_file::<Array>(file.to_owned())
            .map_err(|e| format!("Could not eval config {file:?}: {e}"))?,
    )
    .into_typed_array()
    .map_err(|e| e.to_string().into())
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize, CustomType)]
#[rhai_type(extra = Self::build_rhai)]
pub struct Instance {
    pub transmission: Transmission,
    pub policies: Vec<DeletePolicy>,
}

impl Instance {
    fn build_rhai(builder: &mut TypeBuilder<Self>) {
        builder.with_fn("rules", Self::new);
    }

    pub fn new(transmission: Transmission, policies: Array) -> Result<Self, Box<EvalAltResult>> {
        Ok(Instance {
            transmission,
            policies: Dynamic::from(policies)
                .into_typed_array()
                .map_err(|e| e.to_string())?,
        })
    }
}
