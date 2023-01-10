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
    tx: mpsc::Sender<BusMessage<T>>,
    mix_rx: mpsc::Receiver<BusMessage<T>>,
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
    pub fn add_tx(&mut self) -> BusTx<T> {
        let tx = self.tx.clone();
        BusTx { tx }
    }

    /// Returns a new Receiver that can be used to receive messages.
    pub fn add_rx(&mut self) -> mpsc::Receiver<T> {
        let (tx, rx) = mpsc::channel(self.capacity);
        self.mix_tx.push(tx);
        rx
    }

    /// Starts the message bus. This will continue running until the bus is
    /// closed.
    pub async fn start(&mut self) {
        'out: while let Some(BusMessage::Message(msg)) = self.mix_rx.recv().await {
            for (n, tx) in &mut self.mix_tx.iter().enumerate() {
                match tx.try_send(msg.clone()) {
                    Err(e) => {
                        error!("Failed to send message to queue {}: {}", n, e);
                        break 'out;
                    }
                    _ => (),
                }
            }
            trace!("MessageBus: {:?}", msg);
        }

        debug!("MessageBus: Closed");
        // Close the receiving ends by dropping the senders.
        trace!("Dropping {} senders", self.mix_tx.len());
        self.mix_tx.clear();
        trace!("Senders dropped");
    }
}

pub struct BusTx<T: Debug + Clone + Sync> {
    tx: mpsc::Sender<BusMessage<T>>,
}

impl<T: Debug + Clone + Sync> BusTx<T> {
    pub async fn send(&self, msg: T) -> Result<(), mpsc::error::SendError<T>> {
        self.tx
            .send(BusMessage::Message(msg))
            .await
            .map_err(|mpsc::error::SendError(e)| match e {
                BusMessage::Message(e) => mpsc::error::SendError(e),
                _ => unreachable!(),
            })
    }

    pub async fn close(&self) -> Result<(), mpsc::error::SendError<()>> {
        self.tx
            .send(BusMessage::Close)
            .await
            .map_err(|_| mpsc::error::SendError(()))
    }
}

impl<T: Debug + Clone + Sync> Clone for BusTx<T> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}
