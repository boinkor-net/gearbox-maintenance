use anyhow::bail;
use gearbox_maintenance::config::configure;
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use tempfile::{tempdir, TempDir};

fn build_config(
    main_contents: String,
    values: HashMap<String, String>,
) -> anyhow::Result<(PathBuf, TempDir)> {
    use std::io::prelude::*;

    let tempdir = tempdir()?;
    let main = tempdir.path().join("main.rhai");
    let mut main_fh = File::create(&main)?;
    main_fh.write_all(main_contents.as_bytes())?;

    for (name, contents) in values {
        let mut fh = File::create(tempdir.path().join(name))?;
        fh.write_all(contents.as_bytes())?;
    }
    Ok((main, tempdir))
}

#[test]
fn can_include_configs() -> anyhow::Result<()> {
    let (path, _tmpdir) = build_config(
        r#"
      import "baz" as b;

      [rules(
         transmission(b::url), []
       )
      ]
    "#
        .to_string(),
        HashMap::from([(
            "baz.rhai".to_string(),
            r#"export const url = "bar";"#.to_string(),
        )]),
    )?;
    configure(&path).map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

#[test]
fn noop_and_non_noop_policies() -> anyhow::Result<()> {
    let (path, _tmpdir) = build_config(
        r#"
      [rules(
         transmission("x"),
         [
           delete_policy("should_delete", on_trackers(["foo"]), matching().max_ratio(1.0)),
           noop_delete_policy("should_not_delete", on_trackers(["bar"]), matching().max_ratio(1.0)),
         ]
       )
      ]
    "#
        .to_string(),
        HashMap::from([]),
    )?;
    let instances = configure(&path).map_err(|e| anyhow::anyhow!("{e}"))?;
    if let [inst] = &instances[..] {
        assert!(inst.policies[0].delete_data);
        assert!(!inst.policies[1].delete_data);
    } else {
        bail!("No instances")
    }
    Ok(())
}
