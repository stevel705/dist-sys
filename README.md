# Fly.io Distributed Systems Challenges — Rust

Реализация серии челленджей <https://fly.io/dist-sys/> на Rust поверх собственного минимального рантайма для Maelstrom-протокола.

## Структура

Cargo workspace:

- [`common/`](common/) — общий рантайм:
  - `Message<P>` / `Body<P>` — типы сообщений Maelstrom-протокола (JSON по строке).
  - `Event<P>` — `Message(...)` или `Tick`. Что приходит в `Node::step`.
  - `Node` trait — `from_init`, `step`, опциональный `tick_interval`.
  - `main_loop` — handshake init/init_ok, spawn-ит stdin-тред, главный цикл с `recv_timeout` для синтеза `Event::Tick`.
- [`echo/`](echo/) — Challenge #1.
- [`unique-ids/`](unique-ids/) — Challenge #2.
- [`broadcast/`](broadcast/) — Challenge #3 (5 стадий).

Каждый bin-крейт добавляется в `members` корневого `Cargo.toml`.

## Зависимости

- Rust (stable, edition 2024)
- Java (для Maelstrom-харнесса)
- graphviz, gnuplot (для графиков Maelstrom; gnuplot — `sudo pacman -S gnuplot`)

## Maelstrom

Распакован в `./maelstrom/` (в `.gitignore`). Если потребуется заново:

```bash
curl -L -o maelstrom.tar.bz2 https://github.com/jepsen-io/maelstrom/releases/download/v0.2.3/maelstrom.tar.bz2
tar -xjf maelstrom.tar.bz2 && rm maelstrom.tar.bz2
```

## Запуск

Сборка всего workspace:
```bash
cargo build --release
```

Прогон конкретного challenge — у каждого крейта своя Maelstrom-команда (см. соответствующий README). Базовый пример (echo):
```bash
./maelstrom/maelstrom test -w echo --bin ./target/release/echo \
  --node-count 1 --time-limit 5
```

Веб-отчёт по последнему прогону:
```bash
./maelstrom/maelstrom serve
# http://localhost:8080
```

## Прогресс

Решения зафиксированы git-тегами по стадиям — `git checkout stage-3b` восстановит код, проходивший 3b.

| Challenge                                  | Тег              | Статус |
|--------------------------------------------|------------------|--------|
| #1 Echo                                    | `stage-1-echo`   | ✅     |
| #2 Unique ID Generation                    | `stage-2-unique-ids` | ✅     |
| #3a Broadcast — single-node                | `stage-3a`       | ✅     |
| #3b Broadcast — multi-node                 | `stage-3b`       | ✅     |
| #3c Broadcast — fault-tolerant             | `stage-3c`       | ✅     |
| #3d Broadcast — efficient I                | `stage-3d`       | ✅     |
| #3e Broadcast — efficient II               | `stage-3e`       | ✅ correctness; tail latency partition-bound |
| #4  Grow-Only Counter                      | —                | —      |
| #5a Kafka-Style Log — single               | —                | —      |
| #5b Kafka-Style Log — multi                | —                | —      |
| #5c Kafka-Style Log — efficient            | —                | —      |
| #6a Totally-Available Transactions — single| —                | —      |
| #6b Read-uncommitted                       | —                | —      |
| #6c Read-committed                         | —                | —      |
