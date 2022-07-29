use std::sync::Arc;

use axum::extract::ws::Message;
use axum::extract::Extension;
use axum::extract::WebSocketUpgrade;
use axum::response::IntoResponse;
use axum::Json;
use dashmap::DashSet;
use futures::Sink;
use futures::SinkExt;
use futures::Stream;
use futures::StreamExt;
use tokio::sync::broadcast;
use tokio::time::interval;
use tokio::time::Duration;
use tracing::warn;
use ws_shared::Msg;
use ws_shared::MsgData;
use ws_shared::ServerResp;

const CAPACITY: usize = 64;
const PONG_INTERVAL: u64 = 30;

#[derive(Debug)]
pub struct State {
    online_dids: DashSet<String>,
    tx: broadcast::Sender<Arc<Msg>>,
}

impl Default for State {
    fn default() -> Self {
        let (tx, _rx) = broadcast::channel(CAPACITY);
        Self {
            online_dids: Default::default(),
            tx,
        }
    }
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
where S: Stream<Item = Result<Message, axum::Error>> + Sink<Message> + Send + 'static {
    let (mut sender, mut receiver) = socket.split();

    let dids = &ServerResp::list(state.list(None));
    let data = dids.try_into().unwrap();
    if sender.send(Message::Text(data)).await.is_err() {
        warn!("failed to send list response");
        return;
    };

    let joined_dids = Arc::new(DashSet::new());
    let mut rx = state.tx.subscribe();

    let state1 = state.clone();
    let joined_dids1 = joined_dids.clone();

    let mut recv_task = tokio::spawn(async move {
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
    });

    let mut send_task = tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(PONG_INTERVAL));

        loop {
            tokio::select! {
                ev = rx.recv() => {
                    if let Ok(msg) = ev {
                        let data = msg.as_ref().try_into().unwrap();
                        if sender.send(Message::Text(data)).await.is_err() {
                            warn!("failed to send message");
                            break;
                        }
                    }
                    else {
                        warn!("failed to subscribe from state");
                        break;
                    }
                }
                _ = interval.tick() => {
                    if sender.send(Message::Pong(vec![])).await.is_err() {
                        warn!("failed to send pong");
                        break;
                    }
                }
            }
        }
    });

    // If any of the tasks fail, we need to shut down the other one
    tokio::select! {
        _v1 = &mut recv_task => send_task.abort(),
        _v2 = &mut send_task => recv_task.abort(),
    }

    // This user has left. Should clean all data bound on it.
    warn!("Connection for {:?} closed", joined_dids);
    state.leave(joined_dids.clone());

    for did in joined_dids.iter() {
        if let Err(e) = state.tx.send(Arc::new(Msg::leave(&did))) {
            warn!("failed to send leave message: {e}");
        }
    }
}

async fn handle_message(msg: Msg, state: Arc<State>, joined_dids: Arc<DashSet<String>>) {
    let msg = match msg.data {
        MsgData::Join => {
            state.join(&msg.did);
            joined_dids.insert(msg.did.to_string());
            msg
        }
        MsgData::Leave => {
            let dids: Arc<DashSet<String>> = Arc::new(DashSet::new());
            dids.insert(msg.did.to_string());
            state.leave(dids);
            joined_dids.remove(&msg.did);
            msg
        }
    };

    if let Err(e) = state.tx.send(Arc::new(msg)) {
        warn!("error sending message: {e}");
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use fake_socket::*;
    use ws_shared::ServerRespData;

    use super::*;

    #[tokio::test]
    async fn handle_join_and_leave_should_work() -> Result<()> {
        let (mut client1, mut client2, state) = prepare_connections().await?;

        let msg1 = &Msg::new("carol", MsgData::Join);
        client1.send(Message::Text(msg1.try_into()?))?;

        assert_recv_msg(&mut client1, "carol", MsgData::Join).await?;
        assert_recv_msg(&mut client2, "carol", MsgData::Join).await?;
        assert_list_equal(state.clone(), vec!["alice", "bob", "carol"]);

        let msg2 = &Msg::new("carol", MsgData::Leave);
        client1.send(Message::Text(msg2.try_into()?))?;

        assert_recv_msg(&mut client1, "carol", MsgData::Leave).await?;
        assert_recv_msg(&mut client2, "carol", MsgData::Leave).await?;
        assert_list_equal(state.clone(), vec!["alice", "bob"]);

        Ok(())
    }

    #[tokio::test]
    async fn handle_client_disconnect_should_work() -> Result<()> {
        let (mut client1, mut client2, state) = prepare_connections().await?;

        let msg1 = &Msg::new("carol", MsgData::Join);
        client1.send(Message::Text(msg1.try_into()?))?;

        assert_recv_msg(&mut client1, "carol", MsgData::Join).await?;
        assert_recv_msg(&mut client2, "carol", MsgData::Join).await?;
        assert_list_equal(state.clone(), vec!["alice", "bob", "carol"]);

        drop(client1);
        assert_recv_dids_msg(
            &mut client2,
            vec!["alice".to_string(), "carol".to_string()],
            MsgData::Leave,
        )
        .await?;
        assert_list_equal(state.clone(), vec!["bob"]);

        Ok(())
    }

    async fn prepare_connections() -> Result<(FakeClient<Message>, FakeClient<Message>, Arc<State>)>
    {
        let (mut client1, socket1) = create_fake_connection();
        let (mut client2, socket2) = create_fake_connection();

        let state = Arc::new(State::default());

        // mimic server behavior for client1
        let state1 = state.clone();
        tokio::spawn(async move {
            handle_socket(socket1, state1).await;
        });

        // client1 will get empty dids list from server once it connected
        assert_recv_resp(&mut client1, ServerRespData::List(vec![])).await?;

        // the state of server will not change before client1 join
        let dids = state.list(None);
        assert_eq!(dids.len(), 0);

        // client1 join, the server broadcast a join message
        let msg1 = &Msg::new("alice", MsgData::Join);
        client1.send(Message::Text(msg1.try_into()?))?;
        assert_recv_msg(&mut client1, "alice", MsgData::Join).await?;

        // check the state of server updated
        assert_list_equal(state.clone(), vec!["alice"]);

        // mimic server behavior for client1
        let state1 = state.clone();
        tokio::spawn(async move {
            handle_socket(socket2, state1).await;
        });

        // client2 will get dids list from server
        assert_recv_resp(
            &mut client2,
            ServerRespData::List(vec!["alice".to_string()]),
        )
        .await?;

        // client2 join, the server broadcast a join message
        let msg2 = &Msg::new("bob", MsgData::Join);
        client2.send(Message::Text(msg2.try_into()?))?;
        assert_recv_msg(&mut client1, "bob", MsgData::Join).await?;
        assert_recv_msg(&mut client2, "bob", MsgData::Join).await?;

        // check the state of server updated
        assert_list_equal(state.clone(), vec!["alice", "bob"]);

        // return the clients and state for further tests
        Ok((client1, client2, state))
    }

    fn assert_list_equal(state: Arc<State>, expected: Vec<&str>) {
        let mut dids = state.list(None);
        dids.sort();
        assert_eq!(dids, expected);
    }

    async fn assert_recv_msg(
        client: &mut FakeClient<Message>,
        did: &str,
        data: MsgData,
    ) -> Result<()> {
        if let Some(Message::Text(msg1)) = client.recv().await {
            let msg = Msg::try_from(msg1.as_str())?;
            assert_eq!(msg.did, did);
            assert_eq!(msg.data, data);
        }

        Ok::<_, anyhow::Error>(())
    }

    async fn assert_recv_dids_msg(
        client: &mut FakeClient<Message>,
        dids: Vec<String>,
        data: MsgData,
    ) -> Result<()> {
        let mut expected_dids = dids.clone();
        expected_dids.sort();

        let mut got_dids = vec![];

        for _ in 0..expected_dids.len() {
            if let Some(Message::Text(msg1)) = client.recv().await {
                let msg = Msg::try_from(msg1.as_str())?;
                got_dids.push(msg.did.clone());
                assert_eq!(msg.data, data);
            }
        }

        got_dids.sort();
        assert_eq!(got_dids, expected_dids);

        Ok::<_, anyhow::Error>(())
    }

    async fn assert_recv_resp(
        client: &mut FakeClient<Message>,
        data: ServerRespData,
    ) -> Result<()> {
        if let Some(Message::Text(msg1)) = client.recv().await {
            let msg = ServerResp::try_from(msg1.as_str())?;
            assert_eq!(msg.data, data);
        }

        Ok::<_, anyhow::Error>(())
    }
}
