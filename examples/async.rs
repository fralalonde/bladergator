//! A sample application asynchronously printing metrics to stdout.

#[macro_use]
extern crate dipstick;

use std::thread::sleep;
use std::time::Duration;
use dipstick::*;

fn main() {
    let metrics = metric_scope(to_stdout().with_async_queue(0));

    let counter = metrics.counter("counter_a");
    let timer = metrics.timer("timer_b");

    let subsystem_metrics = metrics.with_name("subsystem");
    let event = subsystem_metrics.marker("event_c");
    let gauge = subsystem_metrics.gauge("gauge_d");

    loop {
        // report some metric values from our "application" loop
        counter.count(11);
        gauge.value(22);

        metrics.counter("ad_hoc").count(4);

        event.mark();
        time!(timer, sleep(Duration::from_millis(5)));
        timer.time(|| sleep(Duration::from_millis(5)));
    }
}
