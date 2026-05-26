use std::io::Write;

use common::{Init, Message, Node, main_loop};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Payload {
    Echo { echo: String },
    EchoOk { echo: String },
}

struct EchoNode {
    next_id: usize,
}

impl Node<Payload> for EchoNode {
    fn from_init(_init: Init) -> anyhow::Result<Self> {
        Ok(Self { next_id: 1 })
    }

    fn step(&mut self, input: Message<Payload>, out: &mut dyn Write) -> anyhow::Result<()> {
        match &input.body.payload {
            Payload::Echo { echo } => {
                let reply = input
                    .clone()
                    .into_reply(Payload::EchoOk { echo: echo.clone() }, &mut self.next_id);
                reply.send(out)?;
            }
            Payload::EchoOk { .. } => {}
        }
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    main_loop::<EchoNode, Payload>()
}
