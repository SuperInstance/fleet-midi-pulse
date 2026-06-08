//! Broadcast channel for tick events and subscriber management.

use crate::event::TickEvent;
use crate::PulseError;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};

/// A subscriber receives tick events from the pulse.
#[derive(Debug)]
pub struct PulseReceiver {
    rx: Receiver<TickEvent>,
}

impl PulseReceiver {
    /// Receive the next tick event, blocking.
    pub fn recv(&self) -> Result<TickEvent, PulseError> {
        self.rx.recv().map_err(|_| PulseError::ChannelClosed)
    }

    /// Try to receive with a timeout.
    pub fn recv_timeout(&self, timeout: std::time::Duration) -> Result<TickEvent, RecvTimeoutError> {
        self.rx.recv_timeout(timeout)
    }

    /// Try to receive without blocking.
    pub fn try_recv(&self) -> Result<TickEvent, mpsc::TryRecvError> {
        self.rx.try_recv()
    }
}

/// Manages subscribers and broadcasts tick events.
#[derive(Debug)]
pub struct SubscriberManager {
    senders: Vec<Sender<TickEvent>>,
}

impl Default for SubscriberManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SubscriberManager {
    /// Create a new subscriber manager.
    pub fn new() -> Self {
        Self {
            senders: Vec::new(),
        }
    }

    /// Subscribe to tick events. Returns a PulseReceiver.
    pub fn subscribe(&mut self) -> PulseReceiver {
        let (tx, rx) = mpsc::channel();
        self.senders.push(tx);
        PulseReceiver { rx }
    }

    /// Broadcast a tick event to all subscribers.
    /// Removes disconnected subscribers.
    pub fn broadcast(&mut self, event: TickEvent) -> usize {
        let mut sent = 0;
        self.senders.retain(|tx| {
            if tx.send(event).is_ok() {
                sent += 1;
                true
            } else {
                false
            }
        });
        sent
    }

    /// Number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.senders.len()
    }

    /// Remove all subscribers.
    pub fn clear(&mut self) {
        self.senders.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::TickEvent;

    #[test]
    fn subscribe_and_receive() {
        let mut mgr = SubscriberManager::new();
        let rx = mgr.subscribe();
        assert_eq!(mgr.subscriber_count(), 1);

        let event = TickEvent::new(0, 0, 0, 0.0);
        mgr.broadcast(event);

        let received = rx.recv().unwrap();
        assert_eq!(received, event);
    }

    #[test]
    fn multiple_subscribers() {
        let mut mgr = SubscriberManager::new();
        let rx1 = mgr.subscribe();
        let rx2 = mgr.subscribe();
        assert_eq!(mgr.subscriber_count(), 2);

        let event = TickEvent::new(42, 1, 0, 0.5);
        let sent = mgr.broadcast(event);
        assert_eq!(sent, 2);

        assert_eq!(rx1.recv().unwrap(), event);
        assert_eq!(rx2.recv().unwrap(), event);
    }

    #[test]
    fn disconnected_subscriber_removed() {
        let mut mgr = SubscriberManager::new();
        let rx = mgr.subscribe();
        drop(rx);

        let event = TickEvent::zero();
        let sent = mgr.broadcast(event);
        assert_eq!(sent, 0);
        assert_eq!(mgr.subscriber_count(), 0);
    }

    #[test]
    fn broadcast_returns_sent_count() {
        let mut mgr = SubscriberManager::new();
        let _rx1 = mgr.subscribe();
        let _rx2 = mgr.subscribe();
        let _rx3 = mgr.subscribe();

        let sent = mgr.broadcast(TickEvent::zero());
        assert_eq!(sent, 3);
    }

    #[test]
    fn clear_removes_all() {
        let mut mgr = SubscriberManager::new();
        mgr.subscribe();
        mgr.subscribe();
        mgr.clear();
        assert_eq!(mgr.subscriber_count(), 0);
    }

    #[test]
    fn try_recv_empty() {
        let mut mgr = SubscriberManager::new();
        let rx = mgr.subscribe();
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn recv_timeout_expires() {
        let mut mgr = SubscriberManager::new();
        let rx = mgr.subscribe();
        let result = rx.recv_timeout(std::time::Duration::from_millis(1));
        assert!(matches!(result, Err(RecvTimeoutError::Timeout)));
    }

    #[test]
    fn default_is_new() {
        let mgr = SubscriberManager::default();
        assert_eq!(mgr.subscriber_count(), 0);
    }
}
