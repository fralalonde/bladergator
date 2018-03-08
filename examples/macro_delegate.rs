//! A sample application sending ad-hoc counter values both to statsd _and_ to stdout.

extern crate dipstick;
#[macro_use] extern crate lazy_static;

use dipstick::*;
use std::time::Duration;

// undeclared root (un-prefixed) metrics
app_metrics!( => {
    // create counter "some_counter"
    pub @Counter ROOT_COUNTER: "root_counter";
    // create counter "root_counter"
    pub @Gauge ROOT_GAUGE: "root_gauge";
    // create counter "root_timer"
    pub @Timer ROOT_TIMER: "root_timer";
});

// public source
app_metrics!(pub PUB_METRICS ="pub_lib_prefix" => {
    // create counter "lib_prefix.some_counter"
    pub @Counter PUB_COUNTER: "some_counter";
});

// undeclared (private) prefixed metrics
//app_metrics!("closed_lib_prefix" => {
//    // create counter "lib_prefix.some_counter"
//    pub @Counter MY_COUNTER: "some_counter";
//});

// declare mod source
app_metrics!(LIB_METRICS ="mod_lib_prefix" => {
    // create counter "mod_lib_prefix.some_counter"
    pub @Counter SOME_COUNTER: "some_counter";
});

// reuse declared source
app_metrics!(LIB_METRICS => {
    // create counter "mod_lib_prefix.another_counter"
    @Counter ANOTHER_COUNTER: "another_counter";
});

fn main() {

    set_global_metrics_receiver(to_stdout());

    loop {
        ROOT_COUNTER.count(123);
        ROOT_TIMER.interval_us(2000000);
        ROOT_GAUGE.value(34534);
        std::thread::sleep(Duration::from_millis(40));
    }
}
