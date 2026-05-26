use std::io::{BufRead, Write};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message<P> {
    pub src: String,
    #[serde(rename = "dest")]
    pub dst: String,
    pub body: Body<P>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Body<P> {
    #[serde(rename = "msg_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<usize>,
    #[serde(flatten)]
    pub payload: P,
}

impl<P: Serialize> Message<P> {
    pub fn send<W: Write + ?Sized>(&self, out: &mut W) -> anyhow::Result<()> {
        serde_json::to_writer(&mut *out, self)?;
        out.write_all(b"\n")?;
        Ok(())
    }
}

impl<P> Message<P> {
    pub fn into_reply(self, payload: P, next_id: &mut usize) -> Message<P> {
        let id = *next_id;
        *next_id += 1;
        Message {
            src: self.dst,
            dst: self.src,
            body: Body {
                id: Some(id),
                in_reply_to: self.body.id,
                payload,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum InitPayload {
    Init {
        node_id: String,
        node_ids: Vec<String>,
    },
    InitOk,
}

pub struct Init {
    pub node_id: String,
    pub node_ids: Vec<String>,
}

pub trait Node<P>: Sized {
    fn from_init(init: Init) -> anyhow::Result<Self>;
    fn step(&mut self, input: Message<P>, out: &mut dyn Write) -> anyhow::Result<()>;
}

pub fn main_loop<N, P>() -> anyhow::Result<()>
where
    N: Node<P>,
    P: DeserializeOwned + Serialize,
{
    let stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();

    let mut lines = stdin.lines();

    let init_line = lines
        .next()
        .context("no init message on stdin")?
        .context("failed to read init line")?;
    let init_msg: Message<InitPayload> =
        serde_json::from_str(&init_line).context("failed to parse init message")?;
    let InitPayload::Init { node_id, node_ids } = init_msg.body.payload else {
        bail!("expected Init, got something else");
    };

    let node = N::from_init(Init {
        node_id: node_id.clone(),
        node_ids,
    })?;

    let init_reply: Message<InitPayload> = Message {
        src: init_msg.dst,
        dst: init_msg.src,
        body: Body {
            id: Some(0),
            in_reply_to: init_msg.body.id,
            payload: InitPayload::InitOk,
        },
    };
    init_reply.send(&mut stdout)?;

    let mut node = node;
    for line in lines {
        let line = line.context("failed to read line from stdin")?;
        let msg: Message<P> = serde_json::from_str(&line)
            .with_context(|| format!("failed to parse message: {line}"))?;
        node.step(msg, &mut stdout)?;
    }
    Ok(())
}
