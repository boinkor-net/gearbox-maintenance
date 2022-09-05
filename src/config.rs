pub mod policy;
mod transmission;

use self::policy::Condition;
use crate::config::policy::DeletePolicy;
use crate::config::transmission::Transmission;
use rhai::{module_resolvers::FileModuleResolver, Array};
use rhai::{serde::from_dynamic, Dynamic, Engine, EvalAltResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub fn configure(file: &Path) -> Result<Vec<Instance>, Box<EvalAltResult>> {
    let mut engine = Engine::new();
    let resolver = FileModuleResolver::new_with_path(file.parent().unwrap_or(&PathBuf::from(".")));
    engine.set_module_resolver(resolver);
    engine
        // Transmission type:
        .register_type_with_name::<Transmission>("Transmission")
        .register_fn("transmission", Transmission::new)
        .register_fn("user", Transmission::with_user)
        .register_fn("password", Transmission::with_password)
        .register_result_fn("poll_interval", Transmission::with_poll_interval)
        // Instance type:
        .register_type_with_name::<Instance>("Instance")
        .register_result_fn("rules", Instance::new)
        // Policies
        .register_result_fn("noop_delete_policy", construct_noop_delete_policy)
        .register_result_fn("delete_policy", construct_real_delete_policy)
        // Conditions
        .register_result_fn("matching", Condition::new)
        .register_fn("max_ratio", Condition::with_max_ratio)
        .register_fn("min_file_count", Condition::with_min_file_count)
        .register_fn("max_file_count", Condition::with_max_file_count)
        .register_result_fn("min_seeding_time", Condition::with_min_seeding_time)
        .register_result_fn("max_seeding_time", Condition::with_max_seeding_time);

    Dynamic::from(
        engine
            .eval_file::<Array>(file.to_owned())
            .map_err(|e| format!("Could not eval: {e}"))?,
    )
    .into_typed_array()
    .map_err(|e| format!("{e}").into())
}

pub fn construct_transmission(d: &Dynamic) -> Result<Transmission, Box<EvalAltResult>> {
    from_dynamic::<Transmission>(d)
}

pub fn construct_condition(c: Condition) -> Condition {
    c
}

pub fn construct_noop_delete_policy(
    name: &str,
    match_when: Condition,
) -> Result<DeletePolicy, Box<EvalAltResult>> {
    Ok(DeletePolicy {
        name: Some(name.to_string()),
        match_when: match_when.sanity_check()?,
        delete_data: false,
    })
}

pub fn construct_real_delete_policy(
    name: &str,
    match_when: Condition,
) -> Result<DeletePolicy, Box<EvalAltResult>> {
    Ok(DeletePolicy {
        name: Some(name.to_string()),
        match_when: match_when.sanity_check()?,
        delete_data: true,
    })
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub transmission: Transmission,
    pub policies: Vec<DeletePolicy>,
}

impl Instance {
    pub fn new(transmission: Transmission, policies: Array) -> Result<Self, Box<EvalAltResult>> {
        Ok(Instance {
            transmission,
            policies: Dynamic::from(policies)
                .into_typed_array()
                .map_err(|e| format!("{e}"))?,
        })
    }
}
