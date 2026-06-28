# Fly.io Distributed Systems Challenges — Rust

Реализация серии челленджей <https://fly.io/dist-sys/> на Rust поверх собственного минимального рантайма для Maelstrom-протокола.

## Структура

Cargo workspace:

- `common/` — рантайм: `Message`, `Body`, `Node` trait, `main_loop`. Парсит JSON-сообщения построчно из stdin, диспатчит в реализацию ноды, пишет ответы в stdout.
- `echo/` — Challenge #1: Echo.

Под каждый следующий челлендж создаётся отдельный bin-крейт и добавляется в `members` корневого `Cargo.toml`.

## Зависимости

- Rust (stable, edition 2024)
- Java (для Maelstrom-харнесса)
- graphviz, gnuplot (для графиков Maelstrom; gnuplot опционально — `sudo pacman -S gnuplot`)

## Maelstrom

Распакован в `./maelstrom/` (в `.gitignore`). Если потребуется заново:

```bash
curl -L -o maelstrom.tar.bz2 https://github.com/jepsen-io/maelstrom/releases/download/v0.2.3/maelstrom.tar.bz2
tar -xjf maelstrom.tar.bz2 && rm maelstrom.tar.bz2
```

## Запуск

Echo (Challenge #1):

```bash
cargo build --release
./maelstrom/maelstrom test -w echo --bin ./target/release/echo \
  --node-count 1 --time-limit 5
```

Веб-отчёт по последнему запуску:

```bash
./maelstrom/maelstrom serve
# http://localhost:8080
```

## Следующие челленджи

cargo new --bin broadcast --vcs none


- [ ] #2 Unique ID Generation (`unique-ids`)
- [ ] #3 Broadcast (`broadcast`, 5 этапов)
- [ ] #4 Grow-Only Counter (`counter`)
- [ ] #5 Kafka-Style Log (`kafka`, 3 этапа)
- [ ] #6 Totally-Available Transactions (`txn`, 3 этапа)
