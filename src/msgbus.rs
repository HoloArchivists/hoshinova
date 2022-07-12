use bus::{Bus, BusReader};
use std::{
    fmt::Debug,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

#[derive(Debug, Clone)]
enum BusMessage<T: Debug + Clone + Sync> {
    Message(T),
    Close,
}

/// MessageBus implements a multi-producer, multi-consumer queue. Each consumer
/// has its own queue, so all consumers will receive the same messages.
pub struct MessageBus<T: Debug + Clone + Sync> {
    bus: Bus<BusMessage<T>>,
    tx: mpsc::SyncSender<BusMessage<T>>,
    rx: mpsc::Receiver<BusMessage<T>>,
    closed: AtomicBool,
}

impl<T: Debug + Clone + Sync> MessageBus<T> {
    /// Creates a new MessageBus with the given capacity.
    pub fn new(size: usize) -> Self {
        let bus = Bus::new(size);
        let (tx, rx) = mpsc::sync_channel(size);
        let closed = AtomicBool::new(false);
        Self {
            bus,
            tx,
            rx,
            closed,
        }
    }

    /// Returns a new BusTrx that can be used to send and receive messages.
    pub fn add_trx(&mut self) -> BusTrx<T> {
        let tx = self.tx.clone();
        let rx = self.bus.add_rx();
        BusTrx { tx, rx }
    }

    /// Returns a closure which can be used to close the MessageBus.
    pub fn add_closer(&mut self) -> impl Fn() -> Result<(), mpsc::SendError<()>> {
        let tx = self.tx.clone();
        move || tx.send(BusMessage::Close).map_err(|_| mpsc::SendError(()))
    }

    /// Starts the message bus. This will block until the bus is closed.
    pub fn start(&mut self) {
        for m in self.rx.iter() {
            self.bus.broadcast(m.clone());
            match m {
                BusMessage::Message(m) => {
                    debug!("Received message: {:?}", m);
                }
                BusMessage::Close => {
                    debug!("Received close message");
                    self.closed.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }
    }
}

pub struct BusTrx<T: Debug + Clone + Sync> {
    tx: mpsc::SyncSender<BusMessage<T>>,
    rx: BusReader<BusMessage<T>>,
}

impl<T: Debug + Clone + Sync> BusTrx<T> {
    /// Send a message to the bus.
    pub fn send(&self, m: T) -> Result<(), mpsc::SendError<T>> {
        self.tx.send(BusMessage::Message(m)).map_err(|e| {
            mpsc::SendError(match e.0 {
                BusMessage::Message(m) => m,
                _ => unreachable!(),
            })
        })
    }

    /// Wait until the bus is closed, up to the given timeout. Returns true if
    /// the bus was closed, false if the timeout was reached.
    pub fn wait_until_closed(&mut self, timeout: std::time::Duration) -> bool {
        let time_end = std::time::Instant::now() + timeout;
        loop {
            let time_left = time_end - std::time::Instant::now();
            match self.rx.recv_timeout(time_left) {
                Err(mpsc::RecvTimeoutError::Timeout) => return false,
                Err(mpsc::RecvTimeoutError::Disconnected) => return true,
                Ok(BusMessage::Close) => return true,
                Ok(_) => continue,
            }
        }
    }

    /// Try to receive a message from the bus.
    pub fn try_recv(&mut self) -> Result<T, mpsc::TryRecvError> {
        match self.rx.try_recv() {
            Ok(BusMessage::Message(m)) => Ok(m),
            Ok(BusMessage::Close) => Err(mpsc::TryRecvError::Disconnected),
            Err(e) => Err(e),
        }
    }

    /// Receive a message from the bus, blocking until one is available.
    pub fn recv(&mut self) -> Result<T, mpsc::RecvError> {
        match self.rx.recv() {
            Ok(BusMessage::Message(m)) => Ok(m),
            Ok(BusMessage::Close) => Err(mpsc::RecvError),
            Err(e) => Err(e),
        }
    }
}
