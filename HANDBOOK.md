# The dipstick handbook
This handbook's purpose is to get you started instrumenting your apps with Dipstick
and give an idea of what's possible.

# Background
Dipstick was born of the desire to build a metrics library that would allow to select from,
switch between and combine multiple backends.
Such a design has multiple benefits:
- simplified instrumentation
- flexible configuration
- easier metrics testing

Because of its Rust nature, performance, safety and ergonomy are also prime concerns. 


## API Overview
Dipstick's API is split between _input_ and _output_ layers.
The input layer provides named metrics such as counters and timers to be used by the application.
The output layer controls how metric values will be recorded and emitted by the configured backend(s).
Input and output layers are decoupled, making code instrumentation independent of output configuration.
Intermediates can also be added between input and output for features or performance characteristics. 

Although this handbook covers input before output, implementation can certainly be performed the other way around.

For more details, consult the [docs](https://docs.rs/dipstick/).


## Metrics Input
A metrics library first job is to help a program collect measurements about its operations.

Dipstick provides a restricted but robust set of _four_ instrument types, taking a stance against 
an application's functional code having to pick what statistics should be tracked for each defined metric.
This helps to enforce contracts with downstream metrics systems and keeps code free of configuration elements.
  
#### Counter
Count number of elements processed, e.g. number of bytes received.

#### Marker 
A monotonic counter. e.g. to record the processing of individual events.
Default aggregated statistics for markers are not the same as those for counters.
Value-less metric also makes for a safer API, preventing values other than 1 from being passed.  

#### Timer
Measure an operation's duration.
Usable either through the time! macro, the closure form or explicit calls to start() and stop().
While timers internal precision are in nanoseconds, their accuracy depends on platform OS and hardware. 
Timer's default output format is milliseconds but is scalable up or down.
 
```rust,skt-run
let app_metrics = metric_scope(to_stdout());
let timer =  app_metrics.timer("my_timer");
time!(timer, {/* slow code here */} );
timer.time(|| {/* slow code here */} );

let start = timer.start();
/* slow code here */
timer.stop(start);

timer.interval_us(123_456);
```
 
### Gauge
An instant observation of a resource's value.
Observation of gauges neither automatic or tied to the output of metrics, 
it must be scheduled independently or called explicitly through the code.

### Names
Each metric must be given a name upon creation.
Names are opaque to the application and are used only to identify the metrics upon output.

Names may be prepended with a namespace by each configured backend.
Aggregated statistics may also append identifiers to the metric's name.

Names should exclude characters that can interfere with namespaces, separator and output protocols.
A good convention is to stick with lowercase alphanumeric identifiers of less than 12 characters.

```rust,skt-run
let app_metrics = metric_scope(to_stdout());
let db_metrics = app_metrics.add_prefix("database");
let _db_timer = db_metrics.timer("db_timer");
let _db_counter = db_metrics.counter("db_counter");
```


### Labels

Some backends (such as Prometheus) allow "tagging" the metrics with labels to provide additional context,
such as the URL or HTTP method requested from a web server.
Dipstick offers the thread-local ThreadLabel and global AppLabel context maps to transparently carry 
metadata to the backends configured to use it.

Notes about labels:
- Using labels may incur a significant runtime cost because 
  of the additional implicit parameter that has to be carried around. 
- Labels runtime costs may be even higher if async queuing is used 
  since current context has to be persisted across threads.
- While internally supported, single metric labels are not yet part of the input API. 
  If this is important to you, consider using dynamically defined metrics or open a GitHub issue!


### Static vs dynamic metrics
  
Metric inputs are usually setup statically upon application startup.

```rust,skt-plain
#[macro_use] 
extern crate dipstick;

use dipstick::*;

metrics!("my_app" => {
    COUNTER_A: Counter = "counter_a";
});

fn main() {
    route_aggregate_metrics(to_stdout());
    COUNTER_A.count(11);
}
```

The static metric definition macro is just `lazy_static!` wrapper.

## Dynamic metrics

If necessary, metrics can also be defined "dynamically", with a possibly new name for every value. 
This is more flexible but has a higher runtime cost, which may be alleviated with caching.

```rust,skt-run
let user_name = "john_day";
let app_metrics = to_log().with_cache(512);
app_metrics.gauge(format!("gauge_for_user_{}", user_name)).value(44);
```
    

## Metrics Output
A metrics library's second job is to help a program emit metric values that can be used in further systems.

Dipstick provides an assortment of drivers for network or local metrics output.
Multiple outputs can be used at a time, each with its own configuration. 

### Types
These output type are provided, some are extensible, you may write your own if you need to.

#### Stream
Write values to any Write trait implementer, including files, stderr and stdout.

#### Log
Write values to the log using the log crate.

### Map
Insert metric values in a map.  

#### Statsd
Send metrics to a remote host over UDP using the statsd format. 

#### Graphite
Send metrics to a remote host over TCP using the graphite format. 

#### TODO Prometheus
Send metrics to a remote host over TCP using the Prometheus JSON or ProtoBuf format.

### Attributes
Attributes change the outputs behavior.

#### Prefixes
Outputs can be given Prefixes. 
Prefixes are prepended to the Metrics names emitted by this output.
With network outputs, a typical use of Prefixes is to identify the network host, 
environment and application that metrics originate from.       

#### Formatting
Stream and Log outputs have configurable formatting that enables usage of custom templates.
Other outputs, such as Graphite, have a fixed format because they're intended to be processed by a downstream system.

#### Buffering
Most outputs provide optional buffering, which can be used to optimized throughput at the expense of higher latency.
If enabled, buffering is usually a best-effort affair, to safely limit the amount of memory that is used by the metrics.

#### Sampling
Some outputs such as statsd also have the ability to sample metrics.
If enabled, sampling is done using pcg32, a fast random algorithm with reasonable entropy.

```rust,skt-fail
let _app_metrics = to_statsd("server:8125")?.with_sampling_rate(0.01);
```


## Intermediates

### Proxy

Because the input's actual _implementation_ depends on the output configuration,
it is necessary to create an output channel before defining any metrics.
This is often not possible because metrics configuration could be dynamic (e.g. loaded from a file),
which might happen after the static initialization phase in which metrics are defined.
To get around this catch-22, Dipstick provides a Proxy which acts as intermediate output, 
allowing redirection to the effective output after it has been set up.

### Bucket

Another intermediate output is the Bucket, which can be used to aggregate metric values. 
Bucket-aggregated values can be used to infer statistics which will be flushed out to

Bucket aggregation is performed locklessly and is very fast.
Count, Sum, Min, Max and Mean are tracked where they make sense, depending on the metric type.

#### Preset bucket statistics

Published statistics can be selected with presets such as `all_stats` (see previous example),
`summary`, `average`.

#### Custom bucket statistics

For more control over published statistics, provide your own strategy:
```rust,skt-run
metrics(aggregate());
set_default_aggregate_fn(|_kind, name, score|
    match score {
        ScoreType::Count(count) => 
            Some((Kind::Counter, vec![name, ".per_thousand"], count / 1000)),
        _ => None
    });
```

#### Scheduled publication

Aggregate metrics and schedule to be periodical publication in the background:
    
```rust,skt-run
use std::time::Duration;

let app_metrics = metric_scope(aggregate());
route_aggregate_metrics(to_stdout());
app_metrics.flush_every(Duration::from_secs(3));
```


### Multi

Like Constructicons, multiple metrics outputs can assemble, creating a unified facade that transparently dispatches 
input metrics to each constituent output. 

```rust,skt-fail,no_run
let _app_metrics = metric_scope((
        to_stdout(), 
        to_statsd("localhost:8125")?.with_namespace(&["my", "app"])
    ));
```

### Queue

Metrics can be recorded asynchronously:
```rust,skt-run
let _app_metrics = metric_scope(to_stdout().queue(64));
```
The async queue uses a Rust channel and a standalone thread.
If the queue ever fills up under heavy load, the behavior reverts to blocking (rather than dropping metrics).


## Facilities


