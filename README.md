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

As some deps require nightly rust, you have to build this project with
a nightly compiler:

```sh
rustup install nightly-2021-12-05
cargo +nightly-2021-12-05 install --git git://github.com/antifuchs/gearbox-maintenance gearbox-maintenance
```

## Configuration

The configuration language is
[Starlark](https://github.com/bazelbuild/starlark), a dialect of
python that is geared towards configuration files.

Here's an example config file:

```py
register_policy(
    transmission=transmission("http://localhost:9091/transmission/rpc", user="transmission", password="secret"),
    policies=[
        delete_policy(
            match=match(
                trackers=["tracker-hostname.horse"], # Only match if torrent is tracked on any of these hostnames

                min_file_count=2  # match only torrents that have >=2 files in them

                # Delete any matching torrent if it is >12h old, and has a ratio of 1.4 or more:
                max_ratio=1.4,
                min_seeding_time="12 hours",

                # Or delete any matching torrent if it's being seeded longer than a year:
                max_seeding_time="365 days",
            ),
            delete_data=True,  # "Delete and trash local data"
        ),
        # You can define more conditions here
    ],
)
```

## Invocation

By default, this tool takes no action: `gearbox-maintenance
config.star` will connect to the transmission instances you specify,
and log what actions it would take.

To have it actually delete data, run `gearbox-maintenance -f config.star`.

The default log level is `gearbox-maintenance=info`. You can increase
logging intensity by setting the environment variable
`RUST_LOG=debug`, but beware: some dependencies are very very
loud. See the [env_logger convention
docs](https://rust-lang-nursery.github.io/rust-cookbook/development_tools/debugging/config_log.html)
for details on how to tune the log level.
