use std::process::{Command, Output};

use perf_event::{Builder, Counter};

pub fn execute_cmd(cmd: &str) -> (u64, Output) {
    let mut counter = Builder::new()
        .kind(perf_event::events::Hardware::CPU_CYCLES)
        .build()
        .unwrap();

    counter.enable().unwrap();
    let output = Command::new(cmd).output()?;
    counter.disable().unwrap();

    (counter.read().unwrap(), output)
}
