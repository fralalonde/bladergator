//! A sample application asynchronously printing metrics to stdout.

extern crate dipstick;

use dipstick::{Input, InputScope, QueuedOutput, Stream};
use std::thread;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    let async_metrics = Stream::to_stdout().queued(100).metrics();
    let counter = async_metrics.counter("counter_a");
    for _ in 0..4 {
        let counter = counter.clone();
        thread::spawn(move || {
            loop {
                // report some metric values from our "application" loop
                counter.count(11);
                sleep(Duration::from_millis(50));
            }
        });
    }
    sleep(Duration::from_secs(5000));
}
