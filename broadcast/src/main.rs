use std::{
    collections::{HashMap, HashSet},
    io::Write,
};

use common::{Init, Message, Node, main_loop};
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

struct BroadcastNode {
    node_id: String,
    next_msg_id: usize,
    messages: HashSet<u64>,                 // Store received messages
    topology: HashMap<String, Vec<String>>, // Store topology information
}

impl BroadcastNode {
    fn send(&mut self, dst: &str, payload: Payload, out: &mut dyn Write) -> anyhow::Result<()> {
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
        msg.send(out)
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
        })
    }
    fn step(&mut self, input: Message<Payload>, out: &mut dyn Write) -> anyhow::Result<()> {
        match &input.body.payload {
            Payload::Broadcast { message } => {
                let is_new = self.messages.insert(*message);
                if is_new {
                    let neighbors: Vec<String> = self
                        .topology
                        .get(&self.node_id)
                        .cloned()
                        .unwrap_or_default();

                    for neighbor in &neighbors {
                        if neighbor != &input.src {
                            self.send(neighbor, Payload::Broadcast { message: *message }, out)?;
                        }
                    }
                }
                if input.src.starts_with("c") {
                    let reply = input.into_reply(Payload::BroadcastOk, &mut self.next_msg_id);
                    reply.send(out)?;
                }
            }
            Payload::Read => {
                let reply = input.into_reply(
                    Payload::ReadOk {
                        messages: self.messages.iter().copied().collect(),
                    },
                    &mut self.next_msg_id,
                );
                reply.send(out)?;
            }
            Payload::Topology { topology } => {
                self.topology = topology.clone();
                let reply = input.into_reply(Payload::TopologyOk, &mut self.next_msg_id);
                reply.send(out)?;
            }
            Payload::BroadcastOk | Payload::ReadOk { .. } | Payload::TopologyOk => {}
        }
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    main_loop::<BroadcastNode, Payload>()
}
