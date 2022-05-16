pub mod policy;
mod transmission;

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use anyhow::Context;
use chrono::Duration;
use starlark::environment::{FrozenModule, GlobalsBuilder, Module};
use starlark::eval::{Evaluator, ReturnFileLoader};
use starlark::starlark_module;
use starlark::syntax::{AstModule, Dialect};
use starlark::values::float::StarlarkFloat;
use starlark::values::none::NoneType;
use starlark::values::{AnyLifetime, Value};

use crate::config::policy::DeletePolicy;
use crate::config::transmission::Transmission;

use self::policy::Condition;

/// Configuration for an instance of this program.
#[derive(Debug, AnyLifetime, Default)]
pub struct Config(RefCell<Vec<Instance>>);

impl Config {
    pub fn configure<P: AsRef<Path>>(path: P) -> anyhow::Result<Vec<Instance>> {
        let mut c = Config::default();
        c.eval(path.as_ref())?;
        Ok(c.0.into_inner())
    }

    fn eval(&mut self, path: &Path) -> anyhow::Result<FrozenModule> {
        let ast = AstModule::parse_file(path, &Dialect::Standard)
            .with_context(|| format!("loading {:?}", &path))?;
        let loads: Vec<(String, FrozenModule)> = ast
            .loads()
            .into_iter()
            .map(|load| {
                let path = PathBuf::from(load);
                Ok((
                    load.to_owned(),
                    self.eval(&path)
                        .with_context(|| format!("loading {:?}", &path))?,
                ))
            })
            .collect::<Result<_, anyhow::Error>>()?;
        let modules = loads.iter().map(|(a, b)| (a.as_str(), b)).collect();
        let globals = GlobalsBuilder::new().with(transmission_config).build();
        let module = Module::new();
        module.set("True", Value::new_bool(true));
        module.set("False", Value::new_bool(false));

        let mut eval = Evaluator::new(&module);
        let loader = ReturnFileLoader { modules: &modules };
        eval.set_loader(&loader);
        eval.extra = Some(self); // TODO: get rid of extra
        eval.eval_module(ast, &globals)?;
        module.freeze()
    }
}

#[starlark_module]
fn transmission_config(builder: &mut GlobalsBuilder) {
    fn transmission(
        url: &str,
        user: Option<&str>,
        password: Option<&str>,
        poll_interval: Option<&str>,
    ) -> anyhow::Result<Transmission> {
        let poll_interval = if let Some(i) = poll_interval {
            Duration::from_std(parse_duration::parse(i)?)?
        } else {
            Duration::minutes(5)
        };
        Ok(Transmission {
            url: url.to_string(),
            user: user.map(|p| p.to_string()),
            password: password.map(|p| p.to_string()),
            poll_interval,
        })
    }

    fn r#match(
        trackers: Vec<&str>,
        min_file_count: Option<i32>,
        max_file_count: Option<i32>,
        max_seeding_time: Option<&str>,
        min_seeding_time: Option<&str>,
        max_ratio: Option<StarlarkFloat>,
    ) -> anyhow::Result<Condition> {
        let max_seeding_time = if let Some(max_seeding_time) = max_seeding_time {
            Some(Duration::from_std(parse_duration::parse(
                max_seeding_time,
            )?)?)
        } else {
            None
        };
        let min_seeding_time = if let Some(min_seeding_time) = min_seeding_time {
            Some(Duration::from_std(parse_duration::parse(
                min_seeding_time,
            )?)?)
        } else {
            None
        };
        Condition {
            trackers: trackers.into_iter().map(String::from).collect(),
            min_file_count,
            max_file_count,
            min_seeding_time,
            max_ratio: max_ratio.map(|f| f.0),
            max_seeding_time,
        }
        .sanity_check()
    }

    fn delete_policy(
        name: Option<&str>,
        r#match: &Condition,
        delete_data: Option<bool>,
    ) -> anyhow::Result<DeletePolicy> {
        Ok(DeletePolicy {
            name: name.map(|n| n.to_string()),
            match_when: r#match.clone(),
            delete_data: delete_data.unwrap_or(false),
        })
    }

    fn register_policy(
        transmission: &Transmission,
        policies: Vec<&DeletePolicy>,
    ) -> anyhow::Result<NoneType> {
        let store = eval.extra.unwrap().downcast_ref::<Config>().unwrap();
        store.0.borrow_mut().push(Instance {
            transmission: transmission.clone(),
            policies: policies.into_iter().cloned().collect(),
        });
        Ok(NoneType)
    }
}

#[derive(PartialEq, Debug)]
pub struct Instance {
    pub transmission: Transmission,
    pub policies: Vec<DeletePolicy>,
}
