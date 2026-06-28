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
    #[warn(dead_code)]
    node_id: String,
    next_msg_id: usize,
    messages: HashSet<u64>,                 // Store received messages
    topology: HashMap<String, Vec<String>>, // Store topology information
}

impl Node<Payload> for BroadcastNode {
    fn from_init(init: Init) -> anyhow::Result<Self> {
        Ok(Self {
            node_id: init.node_id,
            next_msg_id: 1,
            messages: HashSet::new(),
            topology: HashMap::new(),
        })
    }

    fn step(&mut self, input: Message<Payload>, out: &mut dyn Write) -> anyhow::Result<()> {
        match &input.body.payload {
            Payload::Broadcast { message } => {
                self.messages.insert(*message);
                let reply = input.into_reply(Payload::BroadcastOk {}, &mut self.next_msg_id);
                reply.send(out)?;
            }
            Payload::Read {} => {
                let reply = input.into_reply(
                    Payload::ReadOk {
                        messages: self.messages.iter().cloned().collect(),
                    },
                    &mut self.next_msg_id,
                );
                reply.send(out)?;
            }
            Payload::Topology { topology } => {
                self.topology = topology.clone();
                let reply = input.into_reply(Payload::TopologyOk {}, &mut self.next_msg_id);
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
