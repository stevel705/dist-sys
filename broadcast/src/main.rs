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
    Gossip {
        messages: HashSet<u64>,
    },
    GossipOk {
        messages: HashSet<u64>,
    },
}

struct InFlight {
    msgs: HashSet<u64>,
    last_sent: Instant,
}

struct BroadcastNode {
    node_id: String,
    next_msg_id: usize,
    messages: HashSet<u64>,                  // Store received messages
    topology: HashMap<String, Vec<String>>,  // Store topology information
    known_to: HashMap<String, HashSet<u64>>, // Store known messages for each neighbor
    in_flight: HashMap<String, InFlight>,
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
            known_to: HashMap::new(),
            in_flight: HashMap::new(),
        })
    }

    fn step(&mut self, input: Event<Payload>, out: &mut dyn Write) -> anyhow::Result<()> {
        match input {
            Event::Message(msg) => match &msg.body.payload {
                Payload::Broadcast { message } => {
                    let is_new = self.messages.insert(*message);

                    // Eager push only on client broadcasts (нода→нода идёт через Gossip)
                    if is_new && msg.src.starts_with('c') {
                        let neighbors = self
                            .topology
                            .get(&self.node_id)
                            .cloned()
                            .unwrap_or_default();
                        for neighbor in &neighbors {
                            self.send(
                                neighbor,
                                Payload::Gossip {
                                    messages: HashSet::from([*message]),
                                },
                                out,
                            )?;
                            let entry =
                                self.in_flight.entry(neighbor.clone()).or_insert_with(|| {
                                    InFlight {
                                        msgs: HashSet::new(),
                                        last_sent: Instant::now(),
                                    }
                                });
                            entry.msgs.insert(*message);
                            entry.last_sent = Instant::now(); // ← всегда обновлять
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
                Payload::BroadcastOk | Payload::ReadOk { .. } | Payload::TopologyOk => {}
                Payload::Gossip { messages } => {
                    self.messages.extend(messages);
                    self.known_to
                        .entry(msg.src.clone())
                        .or_default()
                        .extend(messages);

                    let messages = messages.clone();

                    let reply = msg.into_reply(
                        Payload::GossipOk { messages: messages },
                        &mut self.next_msg_id,
                    );
                    reply.send(out)?;
                }
                Payload::GossipOk { messages } => {
                    self.known_to
                        .entry(msg.src.clone())
                        .or_default()
                        .extend(messages);

                    if let Some(entry) = self.in_flight.get_mut(&msg.src) {
                        entry.msgs.retain(|x| !messages.contains(x));
                    }
                }
            },
            Event::Tick => {
                let now = Instant::now();
                let ttl = std::time::Duration::from_millis(500);

                let to_send: Vec<(String, HashSet<u64>)> = {
                    let neighbors = self
                        .topology
                        .get(&self.node_id)
                        .cloned()
                        .unwrap_or_default();
                    let mut plan = Vec::new();

                    for neighbor in neighbors {
                        let active = match self.in_flight.get(&neighbor) {
                            Some(in_flight) if now.duration_since(in_flight.last_sent) < ttl => {
                                in_flight.msgs.clone()
                            }
                            _ => HashSet::new(),
                        };

                        let empty;
                        let known: &HashSet<u64> = match self.known_to.get(&neighbor) {
                            Some(k) => k,
                            None => {
                                empty = HashSet::new();
                                &empty
                            }
                        };

                        let diff: HashSet<u64> = self
                            .messages
                            .difference(known) // вычесть подтверждённое
                            .copied()
                            .filter(|x| !active.contains(x)) // вычесть in-flight
                            .collect();

                        if !diff.is_empty() {
                            plan.push((neighbor, diff));
                        }
                    }
                    plan
                };

                for (neighbor, diff) in to_send {
                    let entry =
                        self.in_flight
                            .entry(neighbor.clone())
                            .or_insert_with(|| InFlight {
                                msgs: HashSet::new(),
                                last_sent: now,
                            });
                    entry.msgs.extend(&diff);
                    entry.last_sent = now;

                    self.send(neighbor.as_str(), Payload::Gossip { messages: diff }, out)?;
                }
            }
        }
        Ok(())
    }

    fn tick_interval(&self) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_millis(300))
    }
}

fn main() -> anyhow::Result<()> {
    main_loop::<BroadcastNode, Payload>()
}
