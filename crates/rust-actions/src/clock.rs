use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Instant(u64);

#[derive(Clone)]
pub struct VirtualClock {
    nanos: Arc<AtomicU64>,
}

impl VirtualClock {
    pub fn new() -> Self {
        Self {
            nanos: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn now(&self) -> Instant {
        Instant(self.nanos.load(Ordering::SeqCst))
    }

    pub fn elapsed_since(&self, instant: Instant) -> Duration {
        let now = self.now();
        Duration::from_nanos(now.0.saturating_sub(instant.0))
    }

    pub fn advance(&self, duration: Duration) {
        self.nanos
            .fetch_add(duration.as_nanos() as u64, Ordering::SeqCst);
    }

    pub fn set(&self, duration: Duration) {
        self.nanos
            .store(duration.as_nanos() as u64, Ordering::SeqCst);
    }

    pub fn reset(&self) {
        self.nanos.store(0, Ordering::SeqCst);
    }

    pub fn current(&self) -> Duration {
        Duration::from_nanos(self.nanos.load(Ordering::SeqCst))
    }
}

impl Default for VirtualClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_advance() {
        let clock = VirtualClock::new();
        assert_eq!(clock.current(), Duration::ZERO);

        let start = clock.now();
        clock.advance(Duration::from_secs(1));
        assert_eq!(clock.elapsed_since(start), Duration::from_secs(1));

        clock.advance(Duration::from_millis(500));
        assert_eq!(clock.elapsed_since(start), Duration::from_millis(1500));
    }

    #[test]
    fn test_set() {
        let clock = VirtualClock::new();
        clock.set(Duration::from_secs(100));
        assert_eq!(clock.current(), Duration::from_secs(100));
    }

    #[test]
    fn test_reset() {
        let clock = VirtualClock::new();
        clock.advance(Duration::from_secs(10));
        clock.reset();
        assert_eq!(clock.current(), Duration::ZERO);
    }
}
