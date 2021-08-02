use log::warn;
use parking_lot::RwLock;
use std::{
    any,
    sync::mpsc,
    sync::Arc,
    thread,
    time::{self, Duration},
};

use crate::{ExpectLog, UpdateRate};

pub trait UpdateLoopTarget {
    fn update(&self, delta_time: &Duration);
}

pub struct UpdateLoop {
    interval: time::Duration,
    base: UpdateLoopBuilder,
}

pub type Target = Arc<RwLock<dyn UpdateLoopTarget + Send + Sync>>;
pub struct UpdateLoopBuilder {
    rate: UpdateRate,
    targets: Vec<Target>,
}

pub struct UpdateLoopStopper {
    tx: mpsc::Sender<()>,
    join_handle: thread::JoinHandle<()>,
}

impl UpdateLoop {
    pub fn new() -> UpdateLoopBuilder {
        UpdateLoopBuilder {
            rate: UpdateRate::PerSecond(60),
            targets: Vec::new(),
        }
    }

    pub fn run(self) -> UpdateLoopStopper {
        let targets = self.base.targets;
        let interval = self.interval;
        let rate = self.base.rate;

        let (tx, rx) = mpsc::channel::<()>();

        let join_handle = thread::spawn(move || {
            // base.

            let time_zero: time::Duration = time::Duration::from_secs_f64(0.0);
            let mut lag = time_zero;

            loop {
                match rx.try_recv() {
                    Ok(_) => break,
                    Err(_) => (),
                }

                let begin = time::Instant::now();
                // thread_tps.lock().insert(u, begin);

                for target in targets.iter() {
                    target.read().update(&interval);
                }

                let update_time = begin.elapsed();
                if update_time <= interval {
                    let time_left = interval - update_time;
                    // there is leftover time
                    if lag <= time_zero {
                        // no lag to reduce
                        thread::sleep(time_left);
                    } else {
                        // lag to be reduced
                        if time_left >= lag {
                            // can be fixed in a single update
                            thread::sleep(time_left - lag);
                            lag = time_zero;
                        } else {
                            // cannot --
                            lag -= time_left;
                        }
                    }
                } else {
                    // falling behind
                    lag += update_time - interval;
                    thread::sleep(interval);
                    warn!(
                        "{} with {:?} is behind: {} seconds",
                        any::type_name::<Self>(),
                        rate,
                        lag.as_secs()
                    );
                }
            }
        });

        UpdateLoopStopper { join_handle, tx }
    }
}

impl UpdateLoopBuilder {
    pub fn with_rate(mut self, rate: UpdateRate) -> Self {
        self.rate = rate;
        self
    }

    pub fn with_target(mut self, target: Target) -> Self {
        self.targets.push(target);
        self
    }

    pub fn build(self) -> UpdateLoop {
        UpdateLoop {
            interval: self.rate.to_interval(),
            base: self,
        }
    }
}

impl UpdateLoopStopper {
    pub fn stop(self) {
        self.tx
            .send(())
            .unwrap_or_else(|_| /* already stopped */ ());
        self.join_handle
            .join()
            .expect_log("UpdateLoop thread join failed");
    }
}
