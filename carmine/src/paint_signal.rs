use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use scarlet_ui::{InvalidationKind, Listenable, SubscriptionId};

pub struct PaintSignal {
    next_id: AtomicU32,
    subscribers: Mutex<BTreeMap<SubscriptionId, Arc<dyn Fn() + Send + Sync>>>,
}

impl PaintSignal {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU32::new(0),
            subscribers: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn notify(&self) {
        let subs = self.subscribers.lock().unwrap();
        for cb in subs.values() {
            cb();
        }
    }
}

impl Listenable for PaintSignal {
    fn subscribe_any(&self, callback: Arc<dyn Fn() + Send + Sync>) -> SubscriptionId {
        let id = SubscriptionId::new(self.next_id.fetch_add(1, Ordering::Relaxed));
        self.subscribers.lock().unwrap().insert(id, callback);
        id
    }

    fn unsubscribe(&self, id: SubscriptionId) -> bool {
        self.subscribers.lock().unwrap().remove(&id).is_some()
    }

    fn invalidation_kind(&self) -> InvalidationKind {
        InvalidationKind::Paint
    }
}
