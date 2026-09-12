//! Per-run inbox: BidiAppend exec_client_message (field 2) waits here while RunSSE stays open.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot, Mutex};

use crate::agent_proto::ExecClientMessage;
use crate::agent_wire::LocalRun;

#[derive(Debug, Clone)]
pub enum ClientEvt {
    Exec(ExecClientMessage),
    Heartbeat,
    Cancel,
    StreamClose(u32),
    Throw { id: u32, error: String },
    Kv(crate::agent_wire::KvClientMessage),
    Interaction(crate::agent_wire::InteractionResponse),
}

struct Slot {
    run: Option<LocalRun>,
    tx: Option<mpsc::UnboundedSender<ClientEvt>>,
    buffered: Vec<ClientEvt>,
}

#[derive(Clone, Default)]
pub struct ExecHub {
    inner: Arc<Mutex<HashMap<String, Slot>>>,
}

impl ExecHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn put_run(&self, run: LocalRun) {
        let mut map = self.inner.lock().await;
        let slot = map.entry(run.request_id.clone()).or_insert_with(|| Slot {
            run: None,
            tx: None,
            buffered: Vec::new(),
        });
        slot.run = Some(run);
    }

    pub async fn get_run(&self, request_id: &str) -> Option<LocalRun> {
        self.inner
            .lock()
            .await
            .get(request_id)
            .and_then(|slot| slot.run.clone())
    }

    pub async fn is_local(&self, request_id: &str) -> bool {
        self.inner.lock().await.contains_key(request_id)
    }

    pub async fn push(&self, request_id: &str, evt: ClientEvt) {
        let mut map = self.inner.lock().await;
        let slot = map.entry(request_id.to_owned()).or_insert_with(|| Slot {
            run: None,
            tx: None,
            buffered: Vec::new(),
        });
        if let Some(tx) = slot.tx.as_ref() {
            if tx.send(evt).is_err() {
                slot.buffered.push(ClientEvt::Cancel);
            }
        } else {
            slot.buffered.push(evt);
        }
    }

    pub async fn subscribe(&self, request_id: &str) -> mpsc::UnboundedReceiver<ClientEvt> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut map = self.inner.lock().await;
        let slot = map.entry(request_id.to_owned()).or_insert_with(|| Slot {
            run: None,
            tx: None,
            buffered: Vec::new(),
        });
        for evt in slot.buffered.drain(..) {
            let _ = tx.send(evt);
        }
        slot.tx = Some(tx);
        rx
    }

    pub async fn finish(&self, request_id: &str) {
        self.inner.lock().await.remove(request_id);
    }
}

#[derive(Clone)]
pub struct WaitBridge {
    exec: mpsc::UnboundedSender<(u32, oneshot::Sender<Option<ExecClientMessage>>)>,
    ix: mpsc::UnboundedSender<(
        u32,
        oneshot::Sender<Option<crate::agent_wire::InteractionResponse>>,
    )>,
}

pub fn spawn_wait_bridge(
    hub: ExecHub,
    request_id: String,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> WaitBridge {
    let (exec_tx, mut exec_jobs) =
        mpsc::unbounded_channel::<(u32, oneshot::Sender<Option<ExecClientMessage>>)>();
    let (ix_tx, mut ix_jobs) = mpsc::unbounded_channel::<(
        u32,
        oneshot::Sender<Option<crate::agent_wire::InteractionResponse>>,
    )>();
    tokio::spawn(async move {
        let mut inbox = hub.subscribe(&request_id).await;
        let mut pending = std::collections::HashMap::new();
        let mut pending_ix = std::collections::HashMap::new();
        let mut waiter: Option<(u32, oneshot::Sender<Option<ExecClientMessage>>)> = None;
        let mut ix_waiter: Option<(
            u32,
            oneshot::Sender<Option<crate::agent_wire::InteractionResponse>>,
        )> = None;
        loop {
            tokio::select! {
                job = exec_jobs.recv() => {
                    let Some((id, reply)) = job else { break };
                    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                        let _ = reply.send(None);
                        continue;
                    }
                    if let Some(got) = pending.remove(&id) {
                        let _ = reply.send(Some(got));
                    } else {
                        waiter = Some((id, reply));
                    }
                }
                job = ix_jobs.recv() => {
                    let Some((id, reply)) = job else { break };
                    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                        let _ = reply.send(None);
                        continue;
                    }
                    if let Some(got) = pending_ix.remove(&id) {
                        let _ = reply.send(Some(got));
                    } else {
                        ix_waiter = Some((id, reply));
                    }
                }
                evt = inbox.recv() => {
                    match evt {
                        None => {
                            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                            if let Some((_, reply)) = waiter.take() {
                                let _ = reply.send(None);
                            }
                            if let Some((_, reply)) = ix_waiter.take() {
                                let _ = reply.send(None);
                            }
                            break;
                        }
                        Some(ClientEvt::Cancel) => {
                            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                            if let Some((_, reply)) = waiter.take() {
                                let _ = reply.send(None);
                            }
                            if let Some((_, reply)) = ix_waiter.take() {
                                let _ = reply.send(None);
                            }
                        }
                        Some(ClientEvt::Exec(exec)) => {
                            if let Some((want, reply)) = waiter.take() {
                                if exec.id == want {
                                    let _ = reply.send(Some(exec));
                                } else {
                                    pending.insert(exec.id, exec);
                                    waiter = Some((want, reply));
                                }
                            } else {
                                pending.insert(exec.id, exec);
                            }
                        }
                        Some(ClientEvt::Interaction(resp)) => {
                            if let Some((want, reply)) = ix_waiter.take() {
                                if resp.id == want {
                                    let _ = reply.send(Some(resp));
                                } else {
                                    pending_ix.insert(resp.id, resp);
                                    ix_waiter = Some((want, reply));
                                }
                            } else {
                                pending_ix.insert(resp.id, resp);
                            }
                        }
                        Some(ClientEvt::Throw { id, error }) => {
                            if let Some((want, reply)) = waiter.take() {
                                if id == want {
                                    let _ = reply.send(Some(throw_exec(id, error)));
                                } else {
                                    waiter = Some((want, reply));
                                }
                            }
                        }
                        Some(
                            ClientEvt::Heartbeat | ClientEvt::StreamClose(_) | ClientEvt::Kv(_),
                        ) => {}
                    }
                }
            }
        }
    });
    WaitBridge {
        exec: exec_tx,
        ix: ix_tx,
    }
}

fn throw_exec(id: u32, error: String) -> ExecClientMessage {
    ExecClientMessage {
        id,
        exec_id: String::new(),
        message: Some(crate::agent_proto::exec_client_message::Message::ShellResult(
            crate::agent_proto::ShellResult {
                result: Some(crate::agent_proto::shell_result::Result::SpawnError(
                    crate::agent_proto::ShellSpawnError {
                        command: String::new(),
                        working_directory: String::new(),
                        error,
                    },
                )),
            },
        )),
    }
}

pub async fn wait_via_bridge(bridge: &WaitBridge, id: u32) -> Option<ExecClientMessage> {
    let (reply, rx) = oneshot::channel();
    bridge.exec.send((id, reply)).ok()?;
    rx.await.ok().flatten()
}

pub async fn wait_ix_via_bridge(
    bridge: &WaitBridge,
    id: u32,
) -> Option<crate::agent_wire::InteractionResponse> {
    let (reply, rx) = oneshot::channel();
    bridge.ix.send((id, reply)).ok()?;
    rx.await.ok().flatten()
}

pub async fn recv_exec(
    rx: &mut mpsc::UnboundedReceiver<ClientEvt>,
    want_id: u32,
    timeout: Duration,
    pending: &mut std::collections::HashMap<u32, ExecClientMessage>,
) -> Option<ExecClientMessage> {
    if let Some(got) = pending.remove(&want_id) {
        return Some(got);
    }
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            return None;
        }
        match tokio::time::timeout(left, rx.recv()).await {
            Ok(Some(ClientEvt::Exec(exec))) if exec.id == want_id => return Some(exec),
            Ok(Some(ClientEvt::Exec(exec))) => {
                pending.insert(exec.id, exec);
            }
            Ok(Some(ClientEvt::Heartbeat)) => {}
            // Official waitForExecClientMessage ignores StreamClose until after the
            // result. Treating it as end-of-wait aborts Read/Grep before the host result.
            Ok(Some(ClientEvt::StreamClose(_))) => {}
            Ok(Some(ClientEvt::Throw { id, error })) if id == want_id => {
                return Some(throw_exec(id, error));
            }
            Ok(Some(ClientEvt::Throw { .. })) => {}
            Ok(Some(ClientEvt::Kv(_) | ClientEvt::Interaction(_))) => {}
            Ok(Some(ClientEvt::Cancel)) | Ok(None) => return None,
            Err(_) => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_proto::ExecClientMessage;

    #[tokio::test]
    async fn recv_exec_queues_out_of_order_ids() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        tx.send(ClientEvt::Exec(ExecClientMessage {
            id: 2,
            exec_id: "two".into(),
            message: None,
        }))
        .unwrap();
        tx.send(ClientEvt::Exec(ExecClientMessage {
            id: 1,
            exec_id: "one".into(),
            message: None,
        }))
        .unwrap();
        let mut pending = std::collections::HashMap::new();
        let first = recv_exec(&mut rx, 1, Duration::from_secs(1), &mut pending)
            .await
            .expect("id 1");
        assert_eq!(first.id, 1);
        let second = recv_exec(&mut rx, 2, Duration::from_secs(1), &mut pending)
            .await
            .expect("id 2");
        assert_eq!(second.id, 2);
    }

    #[tokio::test]
    async fn recv_exec_zero_does_not_match_waiter() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        tx.send(ClientEvt::Exec(ExecClientMessage {
            id: 0,
            exec_id: "zero".into(),
            message: None,
        }))
        .unwrap();
        tx.send(ClientEvt::Exec(ExecClientMessage {
            id: 4,
            exec_id: "four".into(),
            message: None,
        }))
        .unwrap();
        let mut pending = std::collections::HashMap::new();
        let got = recv_exec(&mut rx, 4, Duration::from_secs(1), &mut pending)
            .await
            .expect("id 4");
        assert_eq!(got.id, 4);
        assert!(pending.contains_key(&0));
    }

    #[tokio::test]
    async fn recv_exec_other_id_throw_does_not_abort_wait() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        tx.send(ClientEvt::Throw {
            id: 3,
            error: "other tool".into(),
        })
        .unwrap();
        tx.send(ClientEvt::Exec(ExecClientMessage {
            id: 4,
            exec_id: "four".into(),
            message: None,
        }))
        .unwrap();
        let mut pending = std::collections::HashMap::new();
        let got = recv_exec(&mut rx, 4, Duration::from_secs(1), &mut pending)
            .await
            .expect("id 4 after other throw");
        assert_eq!(got.id, 4);
    }

    #[tokio::test]
    async fn recv_exec_matching_throw_ends_wait() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        tx.send(ClientEvt::Throw {
            id: 4,
            error: "aborted".into(),
        })
        .unwrap();
        let mut pending = std::collections::HashMap::new();
        let got = recv_exec(&mut rx, 4, Duration::from_secs(1), &mut pending)
            .await
            .expect("throw becomes host abort result");
        assert!(
            crate::agent_loop::exec_result_text(&got).contains("aborted"),
            "{}",
            crate::agent_loop::exec_result_text(&got)
        );
    }

    #[tokio::test]
    async fn recv_exec_stream_close_does_not_abort_result_wait() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        tx.send(ClientEvt::StreamClose(3)).unwrap();
        tx.send(ClientEvt::Exec(ExecClientMessage {
            id: 3,
            exec_id: "three".into(),
            message: None,
        }))
        .unwrap();
        let mut pending = std::collections::HashMap::new();
        let got = recv_exec(&mut rx, 3, Duration::from_secs(1), &mut pending)
            .await
            .expect("host result after streamClose");
        assert_eq!(got.id, 3);
        assert_eq!(got.exec_id, "three");
    }

    #[tokio::test]
    async fn spawn_wait_bridge_cancel_aborts_waiter() {
        let hub = ExecHub::new();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let jobs = spawn_wait_bridge(hub.clone(), "req-stop".into(), cancel.clone());
        let pusher = hub.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            pusher.push("req-stop", ClientEvt::Cancel).await;
        });
        let got = wait_via_bridge(&jobs, 1).await;
        assert!(got.is_none());
        assert!(cancel.load(std::sync::atomic::Ordering::Relaxed));
    }
}
