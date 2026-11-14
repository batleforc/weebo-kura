//! Per-device rate limits (RFC 0001 §4.4), in this replica's memory: with N
//! replicas a caller can reach N times a limit; the defaults are sized for two.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// `(method, route template, requests per minute)`, the defaults of §4.4 for
/// the routes phase 1 serves. The gate test `every_route_has_a_limit` fails
/// on a route missing here.
pub const LIMITS: &[(&str, &str, u32)] = &[
    ("POST", "/v1/devices", 10),
    ("GET", "/v1/devices", 60),
    ("DELETE", "/v1/devices/{d}", 10),
    ("GET", "/v1/vaults", 60),
    ("POST", "/v1/vaults", 10),
    ("DELETE", "/v1/vaults/{v}", 10),
    ("PUT", "/v1/vaults/{v}/bindings/{b}", 10),
    ("DELETE", "/v1/vaults/{v}/bindings/{b}", 10),
    ("GET", "/v1/admin/status", 60),
    ("POST", "/v1/vaults/{v}/chunks/missing", 600),
    // §4.4 sets a body limit, not a rate, on chunk and blob uploads; reads are 600.
    ("PUT", "/v1/vaults/{v}/chunks/{hash}", 600),
    ("GET", "/v1/vaults/{v}/chunks/{hash}", 600),
    ("PUT", "/v1/vaults/{v}/blobs/{hash}", 600),
    ("GET", "/v1/vaults/{v}/blobs/{hash}", 600),
];

const WINDOW: Duration = Duration::from_secs(60);
/// Tracked (caller, route) pairs at most.
pub const MAX_TRACKED: usize = 100_000;

pub fn limit_of(method: &str, route: &str) -> Option<u32> {
    LIMITS
        .iter()
        .find(|(m, r, _)| *m == method && *r == route)
        .map(|(_, _, n)| *n)
}

/// (caller, method, route template).
type WindowKey = (String, &'static str, &'static str);

#[derive(Default)]
pub struct RateLimiter {
    windows: Mutex<HashMap<WindowKey, (Instant, u32)>>,
}

impl RateLimiter {
    /// Counts one call of `key` (a device id, or a subject before
    /// registration) on a route; `Err(seconds)` when over its limit.
    pub fn check(&self, key: &str, method: &str, route: &str) -> Result<(), u64> {
        let Some(&(m, r, limit)) = LIMITS.iter().find(|(m, r, _)| *m == method && *r == route)
        else {
            return Ok(());
        };
        let now = Instant::now();
        let mut windows = self.windows.lock().unwrap_or_else(PoisonError::into_inner);
        if windows.len() >= MAX_TRACKED {
            windows.retain(|_, (start, _)| now.duration_since(*start) < WINDOW);
        }
        let k = (key.to_owned(), m, r);
        if windows.len() >= MAX_TRACKED && !windows.contains_key(&k) {
            // shortcut: past 100,000 live callers a new one is not counted; a shared store would be needed then.
            return Ok(());
        }
        let entry = windows.entry(k).or_insert((now, 0));
        let elapsed = now.duration_since(entry.0);
        if elapsed >= WINDOW {
            *entry = (now, 0);
        }
        if entry.1 >= limit {
            let wait = WINDOW
                .saturating_sub(now.duration_since(entry.0))
                .as_secs()
                .max(1);
            return Err(wait);
        }
        entry.1 = entry.1.saturating_add(1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_per_key_and_route_and_says_when_to_retry() {
        let l = RateLimiter::default();
        for _ in 0..10 {
            l.check("d1", "POST", "/v1/devices").unwrap();
        }
        let wait = l.check("d1", "POST", "/v1/devices").unwrap_err();
        assert!((1..=60).contains(&wait), "{wait}");
        l.check("d2", "POST", "/v1/devices").unwrap();
        l.check("d1", "GET", "/v1/devices").unwrap();
        l.check("d1", "GET", "/unlimited").unwrap();
        assert_eq!(limit_of("GET", "/v1/vaults"), Some(60));
    }
}
