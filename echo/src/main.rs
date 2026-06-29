use std::io::Write;

use common::{Event, Init, Node, main_loop};
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

    fn step(&mut self, input: Event<Payload>, out: &mut dyn Write) -> anyhow::Result<()> {
        match input {
            Event::Message(msg) => match &msg.body.payload {
                Payload::Echo { echo } => {
                    let echo = echo.clone();
                    let reply = msg.into_reply(Payload::EchoOk { echo }, &mut self.next_id);
                    reply.send(out)?;
                }
                Payload::EchoOk { .. } => {}
            },
            Event::Tick => {}
        }
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    main_loop::<EchoNode, Payload>()
}
