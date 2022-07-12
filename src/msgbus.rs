use bus::{Bus, BusReader};
use crossbeam::channel;
use std::fmt::Debug;
use std::sync::mpsc;

#[derive(Debug, Clone)]
enum BusMessage<T: Debug + Clone + Sync> {
    Message(T),
    Close,
}

/// MessageBus implements a multi-producer, multi-consumer queue. Each consumer
/// has its own queue, so all consumers will receive the same messages.
pub struct MessageBus<T: Debug + Clone + Sync> {
    bus: Bus<BusMessage<T>>,
    tx: channel::Sender<BusMessage<T>>,
    rx: channel::Receiver<BusMessage<T>>,
}

impl<T: Debug + Clone + Sync> MessageBus<T> {
    /// Creates a new MessageBus with the given capacity.
    pub fn new(size: usize) -> Self {
        let bus = Bus::new(size);
        let (tx, rx) = channel::bounded(size);
        Self { bus, tx, rx }
    }

    /// Returns a new BusTx that can be used to send messages.
    pub fn add_tx(&mut self) -> BusTx<T> {
        let tx = self.tx.clone();
        BusTx { tx }
    }

    /// Returns a new BusRx that can be used to receive messages.
    pub fn add_rx(&mut self) -> BusRx<T> {
        let rx = self.bus.add_rx();
        BusRx { rx }
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
                    break;
                }
            }
        }
    }
}

pub struct BusTx<T: Debug + Clone + Sync> {
    tx: channel::Sender<BusMessage<T>>,
}

impl<T: Debug + Clone + Sync> BusTx<T> {
    /// Sends a message to the bus.
    pub fn send(&self, m: T) -> Result<(), channel::SendError<T>> {
        self.tx.send(BusMessage::Message(m)).map_err(|e| {
            channel::SendError(match e.0 {
                BusMessage::Message(m) => m,
                _ => unreachable!(),
            })
        })
    }

    /// Closes the bus.
    pub fn close(&self) -> Result<(), channel::SendError<()>> {
        self.tx
            .send(BusMessage::Close)
            .map_err(|_| channel::SendError(()))
    }
}

pub struct BusRx<T: Debug + Clone + Sync> {
    rx: BusReader<BusMessage<T>>,
}

impl<T: Debug + Clone + Sync> BusRx<T> {
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
