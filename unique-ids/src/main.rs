use std::io::Write;

use common::{Event, Init, Node, main_loop};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Payload {
    Generate {},
    GenerateOk { id: String },
}

struct UniqueIdsNode {
    node_id: String,
    next_seq: usize,
    next_msg_id: usize,
}

impl Node<Payload> for UniqueIdsNode {
    fn from_init(init: Init) -> anyhow::Result<Self> {
        Ok(Self {
            node_id: init.node_id,
            next_seq: 0,
            next_msg_id: 1,
        })
    }

    fn step(&mut self, input: Event<Payload>, out: &mut dyn Write) -> anyhow::Result<()> {
        match input {
            Event::Message(msg) => match msg.body.payload {
                Payload::Generate {} => {
                    let id = format!("{}-{}", self.node_id, self.next_seq);
                    self.next_seq += 1;
                    let reply = msg.into_reply(Payload::GenerateOk { id }, &mut self.next_msg_id);

                    reply.send(out)?;
                }
                Payload::GenerateOk { .. } => {}
            },
            Event::Tick => {}
        }
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    main_loop::<UniqueIdsNode, Payload>()
}
