use std::{
    collections::{HashMap, HashSet},
    io::Write,
    time::Instant,
};

use common::{Event, Init, Message, Node, main_loop};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Payload {
    Broadcast {
        message: u64,
    },
    BroadcastOk,
    Read,
    ReadOk {
        messages: Vec<u64>,
    },
    Topology {
        topology: HashMap<String, Vec<String>>,
    },
    TopologyOk,
}

#[derive(Debug, Clone)]
struct PendingGossip {
    dst: String,
    message: u64,
    last_attempt: Instant,
}

struct BroadcastNode {
    node_id: String,
    next_msg_id: usize,
    messages: HashSet<u64>,                 // Store received messages
    topology: HashMap<String, Vec<String>>, // Store topology information
    pending: HashMap<usize, PendingGossip>,
}

impl BroadcastNode {
    fn send(&mut self, dst: &str, payload: Payload, out: &mut dyn Write) -> anyhow::Result<usize> {
        let msg_id = self.next_msg_id;
        self.next_msg_id += 1;
        let msg = Message {
            src: self.node_id.clone(),
            dst: dst.to_string(),
            body: common::Body {
                id: Some(msg_id),
                in_reply_to: None,
                payload,
            },
        };
        msg.send(out)?;
        Ok(msg_id)
    }
}

impl Node<Payload> for BroadcastNode {
    fn from_init(init: Init) -> anyhow::Result<Self> {
        let current_node_id = init.node_id;
        let default_neighbors: Vec<String> = init
            .node_ids
            .iter()
            .filter(|n| *n != &current_node_id)
            .cloned()
            .collect();
        let mut topology = HashMap::new();
        topology.insert(current_node_id.clone(), default_neighbors);

        Ok(Self {
            node_id: current_node_id,
            next_msg_id: 1,
            messages: HashSet::new(),
            topology,
            pending: HashMap::new(),
        })
    }

    fn step(&mut self, input: Event<Payload>, out: &mut dyn Write) -> anyhow::Result<()> {
        match input {
            Event::Message(msg) => match &msg.body.payload {
                Payload::Broadcast { message } => {
                    let is_new = self.messages.insert(*message);
                    if is_new {
                        let neighbors: Vec<String> = self
                            .topology
                            .get(&self.node_id)
                            .cloned()
                            .unwrap_or_default();

                        for neighbor in &neighbors {
                            if neighbor != &msg.src {
                                let msg_id = self.send(
                                    neighbor,
                                    Payload::Broadcast { message: *message },
                                    out,
                                )?;
                                self.pending.insert(
                                    msg_id,
                                    PendingGossip {
                                        dst: neighbor.clone(),
                                        message: *message,
                                        last_attempt: Instant::now(),
                                    },
                                );
                            }
                        }
                    }

                    let reply = msg.into_reply(Payload::BroadcastOk, &mut self.next_msg_id);
                    reply.send(out)?;
                }
                Payload::Read => {
                    let reply = msg.into_reply(
                        Payload::ReadOk {
                            messages: self.messages.iter().copied().collect(),
                        },
                        &mut self.next_msg_id,
                    );
                    reply.send(out)?;
                }
                Payload::Topology { topology } => {
                    self.topology = topology.clone();
                    let reply = msg.into_reply(Payload::TopologyOk, &mut self.next_msg_id);
                    reply.send(out)?;
                }
                Payload::BroadcastOk => {
                    if let Some(reply_to) = msg.body.in_reply_to {
                        self.pending.remove(&reply_to);
                    }
                }
                Payload::ReadOk { .. } | Payload::TopologyOk => {}
            },
            Event::Tick => {
                let now = Instant::now();
                let pending_ids: Vec<usize> = self
                    .pending
                    .iter()
                    .filter_map(|(&id, pending)| {
                        if now.duration_since(pending.last_attempt)
                            >= std::time::Duration::from_millis(200)
                        {
                            Some(id)
                        } else {
                            None
                        }
                    })
                    .collect();

                for old_id in pending_ids {
                    let Some(p) = self.pending.remove(&old_id) else {
                        continue;
                    };
                    let new_msg_id =
                        self.send(&p.dst, Payload::Broadcast { message: p.message }, out)?;
                    self.pending.insert(
                        new_msg_id,
                        PendingGossip {
                            dst: p.dst,
                            message: p.message,
                            last_attempt: Instant::now(),
                        },
                    );
                }
            }
        }
        Ok(())
    }

    fn tick_interval(&self) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_millis(100))
    }
}

fn main() -> anyhow::Result<()> {
    main_loop::<BroadcastNode, Payload>()
}
