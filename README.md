![Build Status](https://github.com/antifuchs/gearbox-maintenance/actions/workflows/ci.yml/badge.svg) [![Docs](https://docs.rs/gearbox-maintenance/badge.svg)](https://docs.rs/gearbox-maintenance/) [![crates.io](https://img.shields.io/crates/v/gearbox-maintenance.svg)](https://crates.io/crates/gearbox-maintenance)

# A transmission maintenance tool

Say you are downloading very large numbers from the internet which you
then share with others, and say also that those very large numbers
occupy a lot of space on your hard disk drive.

The people who tell you where to find the large number want you to
keep sharing those numbers equitably, and you yourself want that too!
Large numbers however occupy a lot of space, but after a while you can
stop sharing the numbers, and nobody minds that at all (because
hopefully the very large number has propagated enough).

So, this tool helps you keep the disk space usage of seeded torrents
in check by letting you define an amount of time that torrents get
seeded for, or until their minimum ratio requirement has been met. If
so, it'll delete data.

This is similar to
[autoremove-torrents](https://autoremove-torrents.readthedocs.io/),
except it works with the version of transmission that I run (and no
other torrent client).

## Installation

```sh
cargo --git git://github.com/antifuchs/gearbox-maintenance gearbox-maintenance
```

## Configuration

The configuration language is [Rhai](https://rhai.rs/book/), a
scripting language that works pretty well for configuration files.

Here's an example config file:

```py
[
  rules(
      transmission("http://localhost:9091/transmission/rpc")
        .user("transmission")
        .password("secret")
        .poll_interval("20min"),
      [
          delete_policy("horse_seasons",
                        on_trackers(["tracker-hostname.horse"])
                          .min_file_count(2),
                        matching()
                          .max_ratio(2.3)
                          .min_seeding_time("2 days")
                          .min_seeding_time("14 days")),
          delete_policy("horse_episodes",
                        on_trackers(["tracker-hostname.horse"])
                          .max_file_count(1),
                        matching()
                          .max_ratio(2.3)
                          .min_seeding_time("24 hours")
                          .max_seeding_time("3 days"))
      ],
  )
]
```

You can also use rhai's [module
system](https://rhai.rs/book/language/modules/import.html) to import
files in the same directory.

## Invocation

By default, this tool takes no action: `gearbox-maintenance
config.rhai` will connect to the transmission instances you specify,
and log what actions it would take.

To have it actually delete data, run `gearbox-maintenance -f config.rhai`.

The default log level is `gearbox-maintenance=info`. You can increase
logging intensity by setting the environment variable
`RUST_LOG=debug`, but beware: some dependencies are very very
loud. See the [env_logger convention
docs](https://rust-lang-nursery.github.io/rust-cookbook/development_tools/debugging/config_log.html)
for details on how to tune the log level.
