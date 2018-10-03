[![crates.io](https://img.shields.io/crates/v/dipstick.svg)](https://crates.io/crates/dipstick)
[![docs.rs](https://docs.rs/dipstick/badge.svg)](https://docs.rs/dipstick)
[![Build Status](https://travis-ci.org/fralalonde/dipstick.svg?branch=master)](https://travis-ci.org/fralalonde/dipstick)

# dipstick ![a dipstick picture](https://raw.githubusercontent.com/fralalonde/dipstick/master/assets/dipstick_single_ok_horiz_transparent_small.png)

A one-stop shop metrics library for Rust applications with lots of features,  
minimal impact on applications and a choice of output to downstream systems.

## Features

Dipstick is a toolkit to help all sorts of application collect and send out metrics.
As such, it needs a bit of set up to suit one's needs.
Skimming through the handbook [handbook](https://github.com/fralalonde/dipstick/tree/master/handbook) 
should help you get an idea of the possible configurations.

In short, dipstick-enabled apps _can_:

  - Send metrics to console, log, statsd, graphite or prometheus (one or many)
  - Serve metrics over HTTP
  - Locally aggregate the count, sum, mean, min, max and rate of metric values
  - Publish aggregated metrics, on schedule or programmatically
  - Customize output statistics and formatting
  - Define global or scoped (e.g. per request) metrics
  - Statistically sample metrics (statsd)
  - Choose between sync or async operation
  - Choose between buffered or immediate output
  - Switch between metric backends at runtime

For convenience, dipstick builds on stable Rust with minimal, feature-gated dependencies.

### Non-goals

For performance reasons, dipstick will not
- plot graphs
- send alerts
- track histograms

These are all best done by downstream timeseries visualization and monitoring tools.

## Show me the code!

Here's a basic aggregating & auto-publish counter metric:

```$rust,skt-run
let bucket = Bucket::new();
bucket.set_target(Text::output(io::stdout()));
bucket.flush_every(Duration::from_secs(3));
let counter = bucket.counter("counter_a");
counter.count(8)
```

Persistent apps wanting to declare static metrics will prefer using the `metrics!` macro:

```$rust,skt-run
metrics! { METRICS = "my_app" =>
    pub COUNTER: Counter = "my_counter";
}

fn main() {
    METRICS.set_target(Graphite::output("graphite.com:2003").unwrap());
    COUNTER.count(32);
}
```

For sample applications see the [examples](https://github.com/fralalonde/dipstick/tree/master/examples).
For documentation see the [handbook](https://github.com/fralalonde/dipstick/tree/master/handbook).

To use Dipstick in your project, add the following line to your `Cargo.toml`
in the `[dependencies]` section:

```toml
dipstick = "0.7.0"
```

## License

Dipstick is licensed under the terms of the Apache 2.0 and MIT license.

