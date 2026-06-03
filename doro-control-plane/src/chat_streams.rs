use crate::prelude::*;

#[derive(Debug, Clone, Default)]
pub(crate) struct ChatStreamHub {
    senders: Arc<Mutex<HashMap<Uuid, broadcast::Sender<AiChatStreamEvent>>>>,
}

impl ChatStreamHub {
    pub(crate) async fn subscribe(
        &self,
        message_id: Uuid,
    ) -> broadcast::Receiver<AiChatStreamEvent> {
        self.sender(message_id).await.subscribe()
    }

    pub(crate) async fn publish(&self, event: AiChatStreamEvent) {
        let sender = self.sender(event.message_id).await;
        let _ = sender.send(event);
    }

    async fn sender(&self, message_id: Uuid) -> broadcast::Sender<AiChatStreamEvent> {
        let mut senders = self.senders.lock().await;
        senders
            .entry(message_id)
            .or_insert_with(|| broadcast::channel(256).0)
            .clone()
    }
}
