use std::fmt::Debug;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
enum BusMessage<T: Debug + Clone + Sync> {
    Message(T),
    Close,
}

/// MessageBus implements a multi-producer, multi-consumer queue. Each consumer
/// has its own queue, so all consumers will receive the same messages.
pub struct MessageBus<T: Debug + Clone + Sync> {
    capacity: usize,
    tx: mpsc::Sender<T>,
    mix_rx: mpsc::Receiver<T>,
    mix_tx: Vec<mpsc::Sender<T>>,
}

impl<T: Debug + Clone + Sync> MessageBus<T> {
    /// Creates a new MessageBus with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let (tx, mix_rx) = mpsc::channel(capacity);
        Self {
            capacity,
            tx,
            mix_rx,
            mix_tx: vec![],
        }
    }

    /// Returns a new Sender that can be used to send messages.
    pub fn add_tx(&mut self) -> mpsc::Sender<T> {
        self.tx.clone()
    }

    /// Returns a new Receiver that can be used to receive messages.
    pub fn add_rx(&mut self) -> mpsc::Receiver<T> {
        let (tx, rx) = mpsc::channel(self.capacity);
        self.mix_tx.push(tx);
        rx
    }

    /// Closes the MessageBus.
    pub fn close(&mut self) {
        self.mix_rx.close()
    }

    /// Starts the message bus. This will block until the bus is closed.
    pub async fn start(&mut self) {
        loop {
            match self.mix_rx.recv().await {
                Some(msg) => {
                    for tx in &mut self.mix_tx {
                        tx.try_send(msg.clone()).unwrap();
                    }
                    trace!("MessageBus: {:?}", msg);
                }
                None => {
                    debug!("Message bus closed");
                    break;
                }
            }
        }
    }
}
