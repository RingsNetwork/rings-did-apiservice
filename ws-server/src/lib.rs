use axum::{
    extract::{ws::Message, Extension, WebSocketUpgrade},
    response::IntoResponse,
    Json,
};
use dashmap::DashSet;
use futures::{Sink, Stream, StreamExt};
use std::sync::Arc;
use tracing::warn;
use ws_shared::{Msg, MsgData};

#[derive(Debug, Default)]
pub struct State {
    online_dids: DashSet<String>,
}

impl State {
    fn join(&self, did: &str) {
        self.online_dids.insert(did.to_string());
    }

    fn leave(&self, dids: Arc<DashSet<String>>) {
        for did in dids.iter().map(|v| v.clone()) {
            self.online_dids.remove(&did);
        }
    }

    fn list(&self, skip: Option<usize>) -> Vec<String> {
        self.online_dids
            .iter()
            .skip(skip.unwrap_or(0))
            .take(1000)
            .map(|v| v.clone())
            .collect()
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(state): Extension<Arc<State>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

pub async fn list_handler(Extension(state): Extension<Arc<State>>) -> Json<Vec<String>> {
    state.list(None).into()
}

async fn handle_socket<S>(socket: S, state: Arc<State>)
where
    S: Stream<Item = Result<Message, axum::Error>> + Sink<Message> + Send + 'static,
{
    let (mut _sender, mut receiver) = socket.split();
    let joined_dids = Arc::new(DashSet::new());

    let state1 = state.clone();
    let joined_dids1 = joined_dids.clone();

    while let Some(Ok(data)) = receiver.next().await {
        if let Message::Text(msg) = data {
            handle_message(
                msg.as_str().try_into().unwrap(),
                state1.clone(),
                joined_dids1.clone(),
            )
            .await;
        }
    }

    // This user has left. Should clean all data bound on it.
    warn!("Connection for {:?} closed", joined_dids);
    state.leave(joined_dids);
}

async fn handle_message(msg: Msg, state: Arc<State>, joined_dids: Arc<DashSet<String>>) {
    if msg.data == MsgData::Join {
        state.join(&msg.did);
        joined_dids.insert(msg.did);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use fake_socket::*;

    #[tokio::test]
    async fn handle_join_and_disconnect_should_work() -> Result<()> {
        let (client1, client2, state) = prepare_connections().await?;

        assert_eq!(state.list(None).len(), 0);

        let msg1 = &Msg::new("alice", MsgData::Join);
        let msg2 = &Msg::new("bob", MsgData::Join);
        let msg3 = &Msg::new("carol", MsgData::Join);
        let msg4 = &Msg::new("dan", MsgData::Join);

        client1.send(Message::Text(msg1.try_into()?))?;
        client2.send(Message::Text(msg2.try_into()?))?;
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        assert_list_equal(state.clone(), vec!["alice", "bob"]);

        client1.send(Message::Text(msg3.try_into()?))?;
        client2.send(Message::Text(msg4.try_into()?))?;
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        assert_list_equal(state.clone(), vec!["alice", "bob", "carol", "dan"]);

        drop(client1);
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        assert_list_equal(state.clone(), vec!["bob", "dan"]);

        drop(client2);
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        assert_eq!(state.list(None).len(), 0);

        Ok(())
    }

    async fn prepare_connections() -> Result<(FakeClient<Message>, FakeClient<Message>, Arc<State>)>
    {
        let (client1, socket1) = create_fake_connection();
        let (client2, socket2) = create_fake_connection();

        let state = Arc::new(State::default());

        // mimic server behavior
        let state1 = state.clone();
        tokio::spawn(async move {
            handle_socket(socket1, state1).await;
        });

        let state1 = state.clone();
        tokio::spawn(async move {
            handle_socket(socket2, state1).await;
        });

        let dids = state.list(None);
        assert_eq!(dids.len(), 0);

        Ok((client1, client2, state))
    }

    fn assert_list_equal(state: Arc<State>, expected: Vec<&str>) {
        let mut dids = state.list(None);
        dids.sort();
        assert_eq!(dids, expected);
    }
}
