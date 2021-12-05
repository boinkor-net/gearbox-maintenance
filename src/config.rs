mod transmission;

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use anyhow::Context;
use starlark::environment::{FrozenModule, GlobalsBuilder, Module};
use starlark::eval::{Evaluator, ReturnFileLoader};
use starlark::starlark_module;
use starlark::syntax::{AstModule, Dialect};
use starlark::values::none::NoneType;
use starlark::values::AnyLifetime;

use crate::config::transmission::Transmission;

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
        let mut eval = Evaluator::new(&module);
        let mut loader = ReturnFileLoader { modules: &modules };
        eval.set_loader(&mut loader);
        eval.extra = Some(self); // TODO: get rid of extra
        eval.eval_module(ast, &globals)?;
        Ok(module.freeze()?)
    }
}

#[starlark_module]
fn transmission_config(builder: &mut GlobalsBuilder) {
    fn transmission(url: &str, user: Option<&str>, password: Option<&str>) -> Transmission {
        Ok(Transmission {
            url: url.to_string(),
            user: user.map(|p| p.to_string()),
            password: password.map(|p| p.to_string()),
        })
    }

    fn register_policy(transmission: &Transmission) -> NoneType {
        let store = eval.extra.unwrap().downcast_ref::<Config>().unwrap();
        store.0.borrow_mut().push(Instance {
            transmission: transmission.clone(),
        });
        Ok(NoneType)
    }
}

#[derive(PartialEq, Debug)]
pub struct Instance {
    pub transmission: Transmission,
}
