# Broadcast

Challenge #3 из <https://fly.io/dist-sys/>. Реализация gossip-сервиса: ноды получают целочисленные значения от клиентов, обмениваются ими между собой так, чтобы каждая нода в итоге знала все значения, и отдают накопленный список по запросу.

Состоит из пяти стадий — 3a, 3b, 3c, 3d, 3e — с нарастающими требованиями (multi-node, partition tolerance, latency, message count). Один и тот же бинарь должен в идеале проходить все пять; на каждой стадии меняется только Maelstrom-команда.

Подробная схема всего flow (рантайм + broadcast-логика + retry) — в [FLOW.md](FLOW.md).

## Протокол

Maelstrom присылает три типа сообщений:

| Запрос      | Поля                                          | Ответ          | Поля                  |
|-------------|-----------------------------------------------|----------------|-----------------------|
| `broadcast` | `message: <integer>`                          | `broadcast_ok` | —                     |
| `read`      | —                                             | `read_ok`      | `messages: [<int>...]`|
| `topology`  | `topology: {node_id: [neighbor_id, ...]}`     | `topology_ok`  | —                     |

- `broadcast` — «запомни число».
- `read` — «отдай все, что видел».
- `topology` — Maelstrom сообщает, кого считать соседями для gossip-а.

В сообщениях между нодами `src` начинается с `n` (`n0`, `n1`...), у клиентов — с `c` (`c1`, `c2`...).

Между нодами поверх Maelstrom-канала может ходить любой кастомный payload — Maelstrom его не валидирует. В 3d мы добавим свой вариант `Gossip { messages: Vec<u64> }`.

## Стадии

### 3a — Single-node ✅ (`stage-3a`)

**Условия:** одна нода, нет партишенов. In-memory key-value store.

**Подход:** `HashSet<u64>` для значений, `HashMap<String, Vec<String>>` для топологии. На `broadcast` — `insert + reply`, на `read` — собрать `Vec` через `.iter().copied().collect()`, на `topology` — сохранить и ответить пустым ok.

**Команда:**
```bash
cargo build --release
./maelstrom/maelstrom test -w broadcast --bin ./target/release/broadcast \
  --node-count 1 --time-limit 20 --rate 10
```

### 3b — Multi-node, без партишенов ✅ (`stage-3b`)

**Условия:** 5 нод, надёжная сеть. Maelstrom присылает `broadcast` на одну ноду, проверяет что значение видно на любой другой через `read`. Партишенов нет.

**Подход:** flooding gossip. На `broadcast`:
1. Если число уже видела (`HashSet::insert` вернул `false`) — игнор.
2. Иначе вставляет в `messages`, форвардит соседям из топологии (пропуская отправителя).

Дедупликация через `HashSet` гасит циклы.

**Что появилось в коде:**
- Метод `BroadcastNode::send(dst, payload, out)` для инициативных исходящих (не reply).
- Fallback топология в `from_init` из `init.node_ids` (на случай раннего broadcast).
- Различение клиента/ноды по `input.src.starts_with('c')`.

**Команда:**
```bash
./maelstrom/maelstrom test -w broadcast --bin ./target/release/broadcast \
  --node-count 5 --time-limit 20 --rate 10
```

### 3c — Fault-tolerant ✅ (`stage-3c`)

**Условия:** 5 нод + `--nemesis partition`. Сеть рвётся и собирается. Eventual consistency: все значения должны дойти до всех нод после починки сети.

**Подход:** per-message retry с ack-ами.
1. Каждое gossip-сообщение соседу — запись в `pending: HashMap<usize, PendingGossip>` (ключ = `msg_id`).
2. Получение `broadcast_ok` с `in_reply_to` — удаляем из `pending`.
3. Периодический `Event::Tick` — переотправляем устаревшие записи (через паттерн **remove → send → insert** с новым `msg_id`).
4. Ответ `broadcast_ok` теперь идёт **всегда** (и клиенту, и ноде).

**Что появилось в коде:**
- В `common`: `Event<P>` enum (`Message | Tick`), `Node::tick_interval()`, рантайм с stdin-тредом и каналом `mpsc`, `recv_timeout` для синтеза `Tick`.
- В `broadcast`: `pending` поле, обработчик `BroadcastOk` от ноды, реализация `Event::Tick`.
- `tick_interval = 100ms`, retry threshold = 200ms.

**Команда:**
```bash
./maelstrom/maelstrom test -w broadcast --bin ./target/release/broadcast \
  --node-count 5 --time-limit 20 --rate 10 --nemesis partition
```

### 3d — Efficient I 🚧

**Условия:** **25 нод**, `rate 100/s`, `--latency 100` (artificial network latency, без партишенов). Цели от Fly.io:
- `msgs-per-op ≤ 30`
- median latency ≤ 400ms
- max latency ≤ 600ms

3c-стратегия (per-message gossip + per-message retry) **не уложится** — ≈50+ messages-per-op уже без партишена.

**Подход:** anti-entropy gossip с batching и diff-tracking + **eager push** на клиентский broadcast.

Кардинально меняем модель: вместо «увидел → разослал → жду ack» — «увидел → запомнил, разослал диффы соседям на следующем тике, для клиентских сообщений шлём сразу».

```
on Broadcast { message } from <client>:
  messages.insert(message)
  if is_new:
    # Eager push: убираем tick-wait на source
    for neighbor in neighbors:
      send Gossip { messages: {message} } to neighbor
      known_to[neighbor].insert(message)          # optimistic
  reply broadcast_ok                              ← клиенту мгновенно

on Tick:
  for neighbor in neighbors:
    diff = messages - known_to[neighbor]
    if !diff.is_empty():
      send Gossip { messages: diff } to neighbor

on Gossip { messages: batch } from <node>:
  messages.extend(&batch)
  known_to[node].extend(&batch)
  reply GossipOk { messages: batch }

on GossipOk { messages: batch } from <node>:
  known_to[node].extend(batch)
```

**Свойства:**
- **Batched.** Одно gossip-сообщение содержит сразу много значений.
- **Self-healing на тике.** Anti-entropy на Tick компенсирует потерянные eager push-и.
- **Idempotent.** Дубли гасятся `HashSet`.
- **Низкая latency.** Eager push убирает tick-wait на source — пропагация = 1 сетевой hop.

### Топология: full mesh

Этот challenge физически недостижим на grid-топологии Maelstrom (5×5, diameter 8). С `--latency 100` минимум — 8 хопов × 100ms = **800ms** только сетевая часть → max budget 600ms невозможен.

Поэтому в `from_init` строим **full mesh** из `init.node_ids` (каждая нода знает всех остальных как соседей). `Topology`-сообщение от Maelstrom **игнорируем** (строка `self.topology = topology.clone()` закомментирована). Эквивалентно запуску `--topology total`, но не зависит от внешнего флага.

Это **legitimate инженерный выбор** — реальные системы (Cassandra, Consul, Riak) также строят собственный gossip-overlay вместо подчинения внешней топологии. `topology`-сообщение в спецификации Maelstrom — это **hint**, не предписание.

**Команда:**
```bash
./maelstrom/maelstrom test -w broadcast --bin ./target/release/broadcast \
  --node-count 25 --time-limit 20 --rate 100 --latency 100
```

**Статус:** ✅ пройдено по всем бюджетам (`stage-3d`).

| Метрика | Цель | Получено | Запас |
|---------|------|----------|-------|
| `msgs-per-op` | ≤ 30 | **27.05** | ~10% |
| median (0.5) | ≤ 400ms | **81-84ms** | ×5 |
| p95 | — | 96-97ms | |
| p99 | — | 97-98ms | |
| max (1) | ≤ 600ms | **99ms** | ×6 |

Распределение latency почти плоское в районе 99ms — это один сетевой hop при `--latency 100`, без tick-wait благодаря eager push. msgs/op в районе 27 — еager push добавляет 24 fanout × 2000 ops = ~48k server-msgs, плюс client traffic ~4k = ~52k total / 1986 ops = 26.

### 3e — Efficient II

**Условия:** ещё более жёсткие лимиты на latency и/или message count. Цели от Fly.io:
- `msgs-per-op` ≤ 20
- median latency ≤ 1s
- max latency ≤ 2s

(latency-budget мягче 3d, но `msgs-per-op` строже)

**План:** оптимизации, перечисленные выше для 3d, + selective fanout (gossip к подмножеству соседей на каждом тике, не ко всем). Plumtree-style hybrid eager+lazy gossip.

**Статус:** не начато.

## Структура кода

- [`src/main.rs`](src/main.rs) — единственный бинарь, эволюционирует от 3a к 3e. На каждой стадии — git-тег (`stage-3a`, `stage-3b`, ...).
- Снимки решений можно посмотреть через `git show stage-3a:broadcast/src/main.rs`.
- Сквозная схема архитектуры и flow — [FLOW.md](FLOW.md).
