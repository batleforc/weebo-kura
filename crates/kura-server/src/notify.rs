//! Notifications across replicas (RFC 0001 §6.3): a commit `NOTIFY`s its vault
//! and `seq` in its own transaction; each replica holds one `LISTEN`
//! connection and fans events out to its sockets. Notifications are hints: a
//! lost one costs latency, never correctness, since clients pull.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use sqlx::postgres::PgListener;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// The PostgreSQL channel; payloads are `<vault>:<seq>`, `revoked:<device>`
/// or `beat:<replica>`, never a path or content.
pub const CHANNEL: &str = "kura_vault";
/// Signals a new `config_state` row (empty payload): the snapshot loader
/// reloads at once instead of at its next poll.
pub const CONFIG_CHANNEL: &str = "kura_config";
/// Heartbeat period; two missed beats reconnect (§6.3).
pub const HEARTBEAT: Duration = Duration::from_secs(15);
/// Sockets per device (§4.4).
pub const MAX_SOCKETS: usize = 4;
/// Events buffered per replica; a socket that falls further behind is
/// dropped and pulls on reconnect (§6.3, plan D50).
pub const QUEUE: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Change {
        vault: Uuid,
        seq: u64,
    },
    /// The `LISTEN` connection dropped: notifications may be lost, pull.
    Pull,
    /// A device was revoked: its sockets close.
    Revoked {
        device: Uuid,
    },
}

/// What a replica's sockets share.
pub struct Hub {
    tx: broadcast::Sender<Event>,
    sockets: Mutex<HashMap<Uuid, usize>>,
    replica: Uuid,
    /// Cancelled at shutdown, so sockets close and the server can drain.
    pub closing: CancellationToken,
    /// `kura_ws_connections`, when the hub belongs to a served state.
    open: Option<prometheus::IntGauge>,
    /// Woken by a config `NOTIFY` or a lost connection: the snapshot loader
    /// looks at `config_state` now.
    pub config: tokio::sync::Notify,
}

impl Default for Hub {
    fn default() -> Self {
        Self {
            tx: broadcast::channel(QUEUE).0,
            sockets: Mutex::default(),
            replica: Uuid::new_v4(),
            closing: CancellationToken::new(),
            open: None,
            config: tokio::sync::Notify::new(),
        }
    }
}

/// A socket's seat, released on drop.
pub struct Seat<'a> {
    hub: &'a Hub,
    device: Uuid,
}

impl Drop for Seat<'_> {
    fn drop(&mut self) {
        if let Some(g) = &self.hub.open {
            g.dec();
        }
        let mut s = self
            .hub
            .sockets
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(n) = s.get_mut(&self.device) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                s.remove(&self.device);
            }
        }
    }
}

impl Hub {
    /// A hub counting its sockets in `open`.
    pub fn counting(open: prometheus::IntGauge) -> Self {
        Self {
            open: Some(open),
            ..Self::default()
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    /// A seat for one more socket of `device`, unless it holds four already.
    pub fn enter(&self, device: Uuid) -> Option<Seat<'_>> {
        let mut s = self.sockets.lock().unwrap_or_else(PoisonError::into_inner);
        let n = s.entry(device).or_default();
        if *n >= MAX_SOCKETS {
            return None;
        }
        *n = n.saturating_add(1);
        if let Some(g) = &self.open {
            g.inc();
        }
        Some(Seat { hub: self, device })
    }

    fn publish(&self, e: Event) {
        // No subscriber is not an error: no socket is open on this replica.
        drop(self.tx.send(e));
    }

    /// Applies one payload; returns the lag in milliseconds of this
    /// replica's own heartbeat (`beat:<replica>:<sent at, unix ms>`).
    fn handle(&self, payload: &str, last_beat: &mut tokio::time::Instant) -> Option<u64> {
        match payload.split_once(':') {
            Some(("beat", r)) => {
                let (replica, sent) = r.split_once(':').unwrap_or((r, ""));
                if replica != self.replica.to_string() {
                    return None;
                }
                *last_beat = tokio::time::Instant::now();
                let sent: u64 = sent.parse().ok()?;
                return Some(unix_ms().saturating_sub(sent));
            }
            Some(("revoked", d)) => {
                if let Ok(device) = d.parse() {
                    self.publish(Event::Revoked { device });
                }
            }
            Some((v, seq)) => {
                if let (Ok(vault), Ok(seq)) = (v.parse(), seq.parse()) {
                    self.publish(Event::Change { vault, seq });
                }
            }
            None => {}
        }
        None
    }
}

fn unix_ms() -> u64 {
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or_default()
}

/// `NOTIFY` a vault's new head, inside the commit's transaction: delivered
/// only if it commits.
pub async fn notify_change(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    vault: Uuid,
    seq: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_notify($1, $2)")
        .bind(CHANNEL)
        .bind(format!("{vault}:{seq}"))
        .execute(&mut **tx)
        .await
        .map(drop)
}

pub async fn notify_revoked(pool: &sqlx::PgPool, device: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_notify($1, $2)")
        .bind(CHANNEL)
        .bind(format!("revoked:{device}"))
        .execute(pool)
        .await
        .map(drop)
}

/// The replica's `LISTEN` loop until `shutdown`: heartbeats itself every
/// 15 s, reconnects after two missed beats or a lost connection, and then
/// tells every socket to pull. Purges expired tickets on each beat.
pub async fn listen(
    pool: sqlx::PgPool,
    hub: std::sync::Arc<crate::app::AppState>,
    shutdown: CancellationToken,
) {
    let hub_ref = &hub.hub;
    let mut first = true;
    loop {
        let Some(mut listener) = next_listener(&pool, &shutdown).await else {
            break;
        };
        if !first {
            hub_ref.publish(Event::Pull);
            hub_ref.config.notify_one();
        }
        first = false;
        let mut beat = tokio::time::interval(HEARTBEAT);
        let mut last_beat = tokio::time::Instant::now();
        loop {
            tokio::select! {
                () = shutdown.cancelled() => {
                    hub_ref.closing.cancel();
                    return;
                }
                _ = beat.tick() => {
                    if !on_beat(&pool, &hub, last_beat).await {
                        break;
                    }
                }
                n = listener.try_recv() => {
                    if !on_notification(&hub, n, &mut last_beat) {
                        break;
                    }
                }
            }
        }
    }
    hub_ref.closing.cancel();
}

/// Connects, retrying every 5 s; `None` once `shutdown` fires.
async fn next_listener(pool: &sqlx::PgPool, shutdown: &CancellationToken) -> Option<PgListener> {
    loop {
        let connected = tokio::select! {
            () = shutdown.cancelled() => return None,
            l = connect_listener(pool) => l,
        };
        match connected {
            Ok(l) => return Some(l),
            Err(e) => {
                tracing::warn!(error = %e, "LISTEN connection failed, retrying in 5 s");
                tokio::select! {
                    () = shutdown.cancelled() => return None,
                    () = tokio::time::sleep(Duration::from_secs(5)) => {}
                }
            }
        }
    }
}

/// One received notification; false when the connection is lost (reconnect).
fn on_notification(
    hub: &crate::app::AppState,
    n: Result<Option<sqlx::postgres::PgNotification>, sqlx::Error>,
    last_beat: &mut tokio::time::Instant,
) -> bool {
    match n {
        Ok(Some(n)) if n.channel() == CONFIG_CHANNEL => hub.hub.config.notify_one(),
        Ok(Some(n)) => {
            if let Some(ms) = hub.hub.handle(n.payload(), last_beat) {
                hub.metrics.notify_lag_seconds.set(ms as f64 / 1000.0);
            }
        }
        // Lost and re-established by sqlx: notifications may be lost.
        Ok(None) => {
            hub.hub.publish(Event::Pull);
            hub.hub.config.notify_one();
        }
        Err(e) => {
            tracing::warn!(error = %e, "LISTEN connection lost, reconnecting");
            return false;
        }
    }
    true
}

async fn connect_listener(pool: &sqlx::PgPool) -> Result<PgListener, sqlx::Error> {
    let mut l = PgListener::connect_with(pool).await?;
    l.listen_all([CHANNEL, CONFIG_CHANNEL]).await?;
    Ok(l)
}

/// A heartbeat tick: updates the lag gauge, sends the beat; false once two beats are missed (reconnect).
async fn on_beat(
    pool: &sqlx::PgPool,
    hub: &crate::app::AppState,
    last_beat: tokio::time::Instant,
) -> bool {
    // A beat not back yet is lag too: the gauge rises while
    // LISTEN stalls instead of keeping its last good value.
    let late = last_beat.elapsed().saturating_sub(HEARTBEAT).as_secs_f64();
    if late > hub.metrics.notify_lag_seconds.get() {
        hub.metrics.notify_lag_seconds.set(late);
    }
    if last_beat.elapsed() > HEARTBEAT.saturating_mul(2) {
        tracing::warn!("LISTEN heartbeat missed twice, reconnecting");
        return false;
    }
    send_beat(pool, hub).await;
    true
}

/// One heartbeat: a self-addressed `NOTIFY` that also purges tickets, then the epoch check.
async fn send_beat(pool: &sqlx::PgPool, hub: &crate::app::AppState) {
    let sent = sqlx::query_scalar::<_, f64>(
        "SELECT pg_notification_queue_usage() FROM (SELECT pg_notify($1, $2), kura_ws_ticket_purge()) beat",
    )
    .bind(CHANNEL)
    .bind(format!("beat:{}:{}", hub.hub.replica, unix_ms()))
    .fetch_one(pool)
    .await;
    match sent {
        Ok(usage) => hub.metrics.notify_queue_usage.set(usage),
        Err(e) => tracing::warn!(error = %e, "heartbeat not sent"),
    }
    // A promotion the pool did not see as a reconnect (§6.3).
    check_epoch(pool, hub).await;
}

async fn check_epoch(pool: &sqlx::PgPool, hub: &crate::app::AppState) {
    match crate::db::epoch_check(pool).await {
        Ok(0) => {}
        Ok(n) => {
            hub.metrics.vault_epoch_changes_total.inc_by(n);
            tracing::warn!(
                vaults = n,
                "cluster identity changed: vault epochs regenerated"
            );
        }
        Err(e) => tracing::warn!(error = %e, "epoch check failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_sockets_per_device_at_most() {
        let hub = Hub::default();
        let d = Uuid::new_v4();
        let seats: Vec<_> = (0..4).map(|_| hub.enter(d).unwrap()).collect();
        assert!(hub.enter(d).is_none());
        assert!(hub.enter(Uuid::new_v4()).is_some(), "per device");
        drop(seats);
        assert!(hub.enter(d).is_some(), "seats are released");
    }

    #[test]
    fn payloads_become_events() {
        let hub = Hub::default();
        let mut rx = hub.subscribe();
        let mut beat = tokio::time::Instant::now();
        let (v, d) = (Uuid::new_v4(), Uuid::new_v4());
        hub.handle(&format!("{v}:42"), &mut beat);
        hub.handle(&format!("revoked:{d}"), &mut beat);
        hub.handle("garbage", &mut beat);
        let own = format!("beat:{}:{}", hub.replica, unix_ms().saturating_sub(40));
        assert!(hub.handle(&own, &mut beat).is_some_and(|ms| ms >= 40));
        assert_eq!(
            hub.handle(&format!("beat:{}:1", Uuid::new_v4()), &mut beat),
            None
        );
        hub.handle("x:y", &mut beat);
        assert_eq!(rx.try_recv().unwrap(), Event::Change { vault: v, seq: 42 });
        assert_eq!(rx.try_recv().unwrap(), Event::Revoked { device: d });
        assert!(rx.try_recv().is_err());
    }
}
