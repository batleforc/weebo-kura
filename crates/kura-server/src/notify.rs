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
}

impl Default for Hub {
    fn default() -> Self {
        Self {
            tx: broadcast::channel(QUEUE).0,
            sockets: Mutex::default(),
            replica: Uuid::new_v4(),
            closing: CancellationToken::new(),
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
        Some(Seat { hub: self, device })
    }

    fn publish(&self, e: Event) {
        // No subscriber is not an error: no socket is open on this replica.
        drop(self.tx.send(e));
    }

    fn handle(&self, payload: &str, last_beat: &mut tokio::time::Instant) {
        match payload.split_once(':') {
            Some(("beat", r)) if r == self.replica.to_string() => {
                *last_beat = tokio::time::Instant::now()
            }
            Some(("beat", _)) => {}
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
    }
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
        let connected = async {
            let mut l = PgListener::connect_with(&pool).await?;
            l.listen(CHANNEL).await?;
            Ok::<_, sqlx::Error>(l)
        };
        let mut listener = tokio::select! {
            () = shutdown.cancelled() => break,
            l = connected => match l {
                Ok(l) => l,
                Err(e) => {
                    tracing::warn!(error = %e, "LISTEN connection failed, retrying in 5 s");
                    tokio::select! {
                        () = shutdown.cancelled() => break,
                        () = tokio::time::sleep(Duration::from_secs(5)) => continue,
                    }
                }
            },
        };
        if !first {
            hub_ref.publish(Event::Pull);
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
                    if last_beat.elapsed() > HEARTBEAT.saturating_mul(2) {
                        tracing::warn!("LISTEN heartbeat missed twice, reconnecting");
                        break;
                    }
                    let sent = sqlx::query("SELECT pg_notify($1, $2), kura_ws_ticket_purge()")
                        .bind(CHANNEL)
                        .bind(format!("beat:{}", hub_ref.replica))
                        .execute(&pool)
                        .await;
                    if let Err(e) = sent {
                        tracing::warn!(error = %e, "heartbeat not sent");
                    }
                }
                n = listener.try_recv() => match n {
                    Ok(Some(n)) => hub_ref.handle(n.payload(), &mut last_beat),
                    // Lost and re-established by sqlx: notifications may be lost.
                    Ok(None) => hub_ref.publish(Event::Pull),
                    Err(e) => {
                        tracing::warn!(error = %e, "LISTEN connection lost, reconnecting");
                        break;
                    }
                },
            }
        }
    }
    hub_ref.closing.cancel();
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
        hub.handle("x:y", &mut beat);
        assert_eq!(rx.try_recv().unwrap(), Event::Change { vault: v, seq: 42 });
        assert_eq!(rx.try_recv().unwrap(), Event::Revoked { device: d });
        assert!(rx.try_recv().is_err());
    }
}
