use std::{
    ops::Deref,
    time::{Duration, Instant},
};

pub struct Reporter {
    count: u32,
    elapsed: Duration,
    report_timer: Instant,
    report_interval: Duration,

    last_interval: Option<Duration>,
    last_per_second: Option<f64>,
}

pub struct Timer {
    begin: Instant,
}

impl Default for Reporter {
    fn default() -> Self {
        Self::new_with_interval(Duration::from_secs(3))
    }
}

impl Reporter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with_interval(report_interval: Duration) -> Self {
        Self {
            count: 0_u32,
            elapsed: Duration::default(),
            report_timer: Instant::now(),
            report_interval,

            last_interval: None,
            last_per_second: None,
        }
    }

    pub fn begin(&self) -> Timer {
        Timer {
            begin: Instant::now(),
        }
    }

    pub fn end(&mut self, timer: Timer) -> bool {
        self.elapsed += timer.begin.elapsed();
        self.count += 1;

        let should_report = self.should_report();
        if should_report {
            self.reset();
        }
        should_report
    }

    pub fn manual(&mut self, elapsed: Duration) -> bool {
        self.elapsed += elapsed;
        self.count += 1;

        let should_report = self.should_report();
        if should_report {
            self.reset();
        }
        should_report
    }

    pub fn should_report(&self) -> bool {
        self.report_timer.elapsed() >= self.report_interval
    }

    pub fn report_interval(&self) -> Duration {
        self.report_interval
    }

    /* pub fn report(&mut self, interval_name: &str, per_second_name: &str) {
        let avg = self.elapsed / self.count;
        let fps = self.count as f64 / self.report_interval.as_secs_f64();

        log::debug!(
            "Report ({interval}s)\nAVG {interval_name}: {per_second:.2?}\nRESP {per_second_name}: {resp_fps:.2}\nREAL {per_second_name}: {real_fps:.2}",
            interval = self.report_interval.as_secs_f64(),
            per_second = avg,

            resp_fps = 1.0 / avg.as_secs_f64(),
            real_fps = fps,

            interval_name = interval_name,
            per_second_name = per_second_name,
        );
    } */

    pub fn reset(&mut self) {
        let avg = self.elapsed / self.count;
        let fps = self.count as f64 / self.report_interval.as_secs_f64();

        self.count = 0;
        self.elapsed = Duration::default();
        self.report_timer = Instant::now();
        self.last_interval = Some(avg);
        self.last_per_second = Some(fps);
    }

    /* pub fn report_maybe(&mut self, interval_name: &str, per_second_name: &str) {
        if self.should_report() {
            self.report(interval_name, per_second_name);
            self.reset();
        }
    } */

    pub fn last(&self) -> Option<(Duration, f64)> {
        Some((self.last_interval?, self.last_per_second?))
    }

    pub fn last_string(&self) -> (String, String) {
        (
            self.last_interval
                .map(|ft| format!("{:3.3?}", ft))
                .unwrap_or_else(|| "...".into()),
            self.last_per_second
                .map(|fps| format!("{:3.3}", fps))
                .unwrap_or_else(|| "...".into()),
        )
    }
}

impl Deref for Timer {
    type Target = Instant;

    fn deref(&self) -> &Self::Target {
        &self.begin
    }
}
