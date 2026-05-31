# Echo challenge — разбор кода

Подробный walkthrough по `common/src/lib.rs` и `echo/src/main.rs` для тех, кто знает Python и пишет на Rust первый раз. Python-аналогии даны в скобках.

## Общая картина: что вообще происходит

Maelstrom — это тестовый харнесс. Он:
1. Запускает твой бинарь как обычный процесс.
2. Шлёт в его **stdin** JSON-сообщения (по одному в строке).
3. Читает из его **stdout** ответы (тоже построчно).
4. Самое первое сообщение — `init` со списком нод. Ты обязан ответить `init_ok`.
5. Дальше летят рабочие сообщения (для echo — `echo`, ответ `echo_ok`).

Наш Rust-код = простой цикл: «прочитал строку → распарсил JSON → выдал ответ». Всё.

Формат сообщения такой:
```json
{"src": "c1", "dest": "n1", "body": {"type": "echo", "msg_id": 1, "echo": "hi"}}
```
`body.type` — дискриминатор (какой это вид сообщения). Остальные поля внутри `body` зависят от типа.

---

## `common/src/lib.rs` — рантайм

### 1. Импорты

```rust
use std::io::{BufRead, Write};
use anyhow::{Context, bail};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
```

- `std::io::{BufRead, Write}` — это **трейты** (≈ интерфейсы / Python protocols). `BufRead` даёт `.lines()`, `Write` даёт `.write_all()`. Чтобы вызывать эти методы, трейт должен быть в скоупе — в Rust методы трейта «приходят» только через `use`.
- `anyhow` — крейт для удобной работы с ошибками. `anyhow::Result<T>` ≡ `Result<T, anyhow::Error>` — универсальный тип ошибки, в который можно завернуть что угодно. `Context` — расширение, дающее `.context("сообщение")?` для добавления контекста к ошибке (≈ `raise ... from e` с пояснением). `bail!("...")` — макрос «вернуть Err с этим сообщением» (≈ `raise Exception("...")`).
- `serde` — сериализация. `Serialize` / `Deserialize` — трейты, которые мы навешиваем через `#[derive(...)]`. `DeserializeOwned` — версия `Deserialize` без заимствованных ссылок (нужен, когда тип владеет всеми данными).

### 2. `Message<P>` и `Body<P>` — структура сообщения

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message<P> {
    pub src: String,
    #[serde(rename = "dest")]
    pub dst: String,
    pub body: Body<P>,
}
```

- `pub struct` — публичная структура (≈ Python `@dataclass`).
- `<P>` — **дженерик-параметр**. `Message` параметризован типом тела `P` (это enum конкретного challenge, например `echo::Payload`). По-питоновски: `Message[P]` где `P` — TypeVar. Зачем? Чтобы общий рантайм мог принимать сообщения с _любым_ payload, а каждый challenge определял свой.
- `#[derive(...)]` — макрос, который **автогенерит** реализации перечисленных трейтов:
  - `Debug` → `{:?}` форматирование для отладки (≈ `__repr__`).
  - `Clone` → метод `.clone()` (≈ `copy.deepcopy`).
  - `Serialize` / `Deserialize` → код для JSON ⇄ структура (≈ `pydantic.BaseModel`).
- `#[serde(rename = "dest")]` — в JSON поле зовётся `"dest"`, а в Rust я хочу `dst` (короче). serde сам сделает маппинг.

```rust
pub struct Body<P> {
    #[serde(rename = "msg_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<usize>,
    #[serde(flatten)]
    pub payload: P,
}
```

- `Option<usize>` — это enum `Some(value) | None` (≈ Python `Optional[int]`). `usize` — беззнаковое целое размером с указатель (на 64-битке = u64).
- `skip_serializing_if = "Option::is_none"` — при сериализации в JSON: если поле `None`, не выводить его вообще. (Иначе в JSON попадёт `"msg_id": null`.)
- `#[serde(flatten)]` — **ключевая фишка**. Поля `P` встраиваются прямо в `body`, а не вкладываются как `"payload": {...}`. Т.е. JSON-сообщение `{"type":"echo", "msg_id":1, "echo":"hi"}` парсится так: `msg_id` → `Body.id`, а `type` + `echo` → `Body.payload`. Без `flatten` пришлось бы писать `{"body": {"msg_id":1, "payload": {...}}}` — а Maelstrom такого не присылает.

### 3. `impl Message<P>` — методы

```rust
impl<P: Serialize> Message<P> {
    pub fn send<W: Write + ?Sized>(&self, out: &mut W) -> anyhow::Result<()> {
        serde_json::to_writer(&mut *out, self)?;
        out.write_all(b"\n")?;
        Ok(())
    }
}
```

- `impl<P: Serialize> Message<P>` — «методы для `Message<P>`, **при условии** что `P: Serialize`» (≈ bounded TypeVar). Метод `send` существует только для тех `P`, которые умеют сериализоваться.
- `<W: Write + ?Sized>` — функция дженерик по `W`. `W: Write` — `W` реализует трейт `Write`. `+ ?Sized` — позволяем `W` быть «нетипизированной величины», т.е. dyn-объектом (`dyn Write`). По умолчанию все generic-параметры предполагаются `Sized`, мы это явно ослабляем.
- `&self` — заимствуем self по чтению (≈ Python `self`, но Rust явно различает чтение/запись).
- `&mut W` — мутабельная ссылка на writer (≈ передача объекта, в который будем писать).
- `-> anyhow::Result<()>` — возвращаем `Result<(), anyhow::Error>`. `()` — unit, аналог `None` / `void` (когда возвращать нечего, но надо что-то).
- `serde_json::to_writer(...)?` — серилизует self в JSON и пишет в out. `?` — **оператор «протолкни ошибку»**: если результат `Err(e)`, функция тут же возвращает `Err(e.into())`. (≈ `try/except` который немедленно re-raise, но компактно.)
- `out.write_all(b"\n")?` — дописываем перевод строки. `b"\n"` — байтовая строка (`bytes` в Python).
- `&mut *out` — про это: `out: &mut W`. `*out` — разыменование (получили сам `W`). `&mut *out` — взяли свежую мутабельную ссылку. Зачем? Это техника **reborrow** — позволяет передать ссылку в чужую функцию, не «потеряв» исходную. Можно почти всегда не задумываться, компилятор подсказывает когда нужно.

```rust
impl<P> Message<P> {
    pub fn into_reply(self, payload: P, next_id: &mut usize) -> Message<P> {
        let id = *next_id;
        *next_id += 1;
        Message {
            src: self.dst,
            dst: self.src,
            body: Body { id: Some(id), in_reply_to: self.body.id, payload },
        }
    }
}
```

- Имя `into_reply` — конвенция Rust: `into_X` **поглощает** self (`self` без `&`), потребляет его и возвращает другой объект. По-питоновски как `dataclasses.replace(...)` — но Rust буквально перемещает поля старого `Message` в новый, без копий.
- Меняем местами `src`/`dst` (адресат становится отправителем) и ставим `in_reply_to = self.body.id`.
- `next_id: &mut usize` — мутабельная ссылка на счётчик, чтобы инкрементить наружный state. (Python отдал бы это через `nonlocal` или внутри объекта; в Rust это явная ссылка.)

### 4. `InitPayload` и handshake

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum InitPayload {
    Init { node_id: String, node_ids: Vec<String> },
    InitOk,
}
```

- `enum` в Rust — **алгебраический тип** (не как в Python). Каждый вариант может нести свои поля (≈ `typing.Union[InitMsg, InitOkMsg]` где `InitMsg` — структура с полями).
- `#[serde(tag = "type", rename_all = "snake_case")]` — **самая важная магия для всего challenge**:
  - `tag = "type"` — в JSON есть поле `"type"`, которое говорит какой это вариант enum-а. (≈ tagged union / discriminated union.)
  - `rename_all = "snake_case"` — `InitOk` в JSON станет `"init_ok"`, `Init` → `"init"`.
- enum не `pub` — он используется только внутри `main_loop`, наружу не виден.

```rust
pub struct Init {
    pub node_id: String,
    pub node_ids: Vec<String>,
}
```
Простая структура, которую отдаём в `from_init` ноды (распакованные поля из `InitPayload::Init`).

### 5. `Node` trait

```rust
pub trait Node<P>: Sized {
    fn from_init(init: Init) -> anyhow::Result<Self>;
    fn step(&mut self, input: Message<P>, out: &mut dyn Write) -> anyhow::Result<()>;
}
```

- Trait ≈ Python `Protocol` или `abc.ABC` — описывает интерфейс. Тип-реализатор должен предоставить `from_init` и `step`.
- `<P>` — нода работает с конкретным payload-типом.
- `: Sized` — все реализаторы должны быть фиксированного размера в памяти (стандартно почти всегда так, кроме dyn-trait объектов).
- `from_init(init) -> Self` — **ассоциированная функция** (≈ classmethod / фабрика). `Self` — «тип, реализующий этот trait»: для `EchoNode` это `EchoNode`.
- `step(&mut self, input, out)` — обработчик одного входящего сообщения. `&mut self` — нода может менять своё состояние (`self.next_id += 1`). `&mut dyn Write` — динамический writer (компилятор не знает заранее, что это — stdout, файл, буфер). `dyn` = виртуальный вызов через vtable (≈ обычный Python-полиморфизм).

### 6. `main_loop` — сердце рантайма

```rust
pub fn main_loop<N, P>() -> anyhow::Result<()>
where
    N: Node<P>,
    P: DeserializeOwned + Serialize,
{
```

- Дженерик-функция: какую ноду `N` запускать и какой payload `P` парсить. Параметры передаются на вызове через турбофиш: `main_loop::<EchoNode, Payload>()`.
- `where` — это тот же `<N: Node<P>, P: DeserializeOwned + Serialize>`, просто более читаемо когда границ много.

```rust
    let stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();
    let mut lines = stdin.lines();
```
- `.lock()` — берём эксклюзивный лок (stdin/stdout shared между тредами в Rust по умолчанию). Возвращает `StdinLock`/`StdoutLock` с буферизацией.
- `stdin.lines()` — итератор по строкам (`Iterator<Item = Result<String, io::Error>>`). Ленивый — читает по мере запроса. `let mut` потому что итератор мутабельный (изменяется при `.next()`).

```rust
    let init_line = lines
        .next()
        .context("no init message on stdin")?
        .context("failed to read init line")?;
```
- `lines.next()` → `Option<Result<String, io::Error>>` (≈ может не быть строки, а если есть — может быть ошибка ввода).
- Первый `.context(...)?` — превращает `Option<T>` в `Result<T, anyhow::Error>` (если `None` — ошибка). После `?` извлекаем `T = Result<String, io::Error>`.
- Второй `.context(...)?` — превращает `Result<String, io::Error>` в `Result<String, anyhow::Error>` с контекстом, после `?` имеем `String`.

```rust
    let init_msg: Message<InitPayload> =
        serde_json::from_str(&init_line).context("failed to parse init message")?;
```
- Указываем тип явно (`: Message<InitPayload>`), чтобы serde знал куда парсить (≈ `pydantic.parse_obj_as(Message[InitPayload], ...)`).

```rust
    let InitPayload::Init { node_id, node_ids } = init_msg.body.payload else {
        bail!("expected Init, got something else");
    };
```
- **let-else**: «попробуй раскрыть в этот паттерн; если не получилось — выполни else, который обязан `return` / `panic` / `bail`». Здесь: если payload — действительно `Init`, то локально появятся переменные `node_id` и `node_ids`. Иначе — выходим с ошибкой.

```rust
    let node = N::from_init(Init { node_id: node_id.clone(), node_ids })?;
```
- Зовём фабрику ноды. `N::from_init(...)` — вызов ассоциированной функции трейта.
- `.clone()` для `node_id` — потому что мы передаём её в `Init` (по значению, владение уходит), но ниже она нам ещё понадобится. Хотя — глядя на код — она дальше не используется, можно убрать `.clone()`.

```rust
    let init_reply: Message<InitPayload> = Message { ... };
    init_reply.send(&mut stdout)?;
```
- Конструируем ответ `init_ok` руками (не через `into_reply`, потому что у нас payload-типа `InitPayload`, а `into_reply` остался бы один к одному — можно через него, но конкретно тут проще явно).

```rust
    let mut node = node;
    for line in lines {
        let line = line.context("failed to read line from stdin")?;
        let msg: Message<P> = serde_json::from_str(&line)
            .with_context(|| format!("failed to parse message: {line}"))?;
        node.step(msg, &mut stdout)?;
    }
    Ok(())
}
```
- `let mut node = node;` — пересоздаём binding как мутабельный (метод `step` требует `&mut self`). Можно было сразу написать `let mut node = N::from_init(...)?;` выше — это микро-чистка.
- Цикл: каждая строка → парсим в `Message<P>` (где `P` — payload challenge-а) → дёргаем `node.step(...)`.
- `with_context(|| ...)` — ленивый вариант `context`: строка форматируется только если случилась ошибка (не на каждое сообщение). `||` — замыкание без аргументов (≈ `lambda: ...`).
- `Ok(())` — успех, ничего не возвращаем.

---

## `echo/src/main.rs` — challenge

### 1. Payload-enum

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Payload {
    Echo { echo: String },
    EchoOk { echo: String },
}
```
- Описание всех типов сообщений, которые этот challenge видит/шлёт. JSON-shape ровно такой, как требует Maelstrom: `{"type":"echo","echo":"..."}` ↔ `Payload::Echo { echo: "..." }`.

### 2. Структура ноды + реализация трейта

```rust
struct EchoNode {
    next_id: usize,
}

impl Node<Payload> for EchoNode {
    fn from_init(_init: Init) -> anyhow::Result<Self> {
        Ok(Self { next_id: 1 })
    }
```
- `EchoNode` — состояние ноды. Для echo нужен только счётчик исходящих `msg_id`.
- `_init` с подчёркиванием — параметр, который мы не используем (echo не требует знать список нод). Подчёркивание глушит warning о неиспользуемом аргументе.
- `Ok(Self { next_id: 1 })` — `Self` внутри `impl` равно `EchoNode`. Возвращаем успешный `Result` со свеженькой нодой.

```rust
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
```
- `match &input.body.payload` — берём ссылку на payload, чтобы не «съесть» `input` (он ещё нужен для `into_reply`).
- `Payload::Echo { echo } => { ... }` — паттерн-матчинг с распаковкой: получаем переменную `echo: &String`.
- `input.clone().into_reply(...)` — клонируем `input`, потому что `into_reply` потребляет `self` (забирает владение). Без `.clone()` мы потеряли бы `input` — здесь не критично, но обычно нода может ещё что-то с ним делать. Можно микро-оптимизировать (`into_reply` достаточно умный, чтобы передвинуть владение без клона), но это шум на старте.
- `echo.clone()` — `echo` это `&String` (ссылка). Чтобы положить в новый payload `String` (по значению), клонируем строку.
- `Payload::EchoOk { .. }` — паттерн «вариант EchoOk, остальные поля игнорируем». В echo-challenge клиент нам `echo_ok` не шлёт, но enum это потенциально допускает — пустая ветка молча игнорирует.

### 3. `main`

```rust
fn main() -> anyhow::Result<()> {
    main_loop::<EchoNode, Payload>()
}
```
- Турбофиш `::<EchoNode, Payload>` — «вот эти два дженерик-параметра подставь». Компилятор не может вывести их сам, потому что в `main_loop` нет аргументов, по которым он мог бы догадаться.
- `main` возвращает `Result<()>` — если ошибка, Rust сам распечатает её и завершит с кодом 1.

---

## Питон-перевод для интуиции

Если бы это был Python с pydantic + abc:

```python
from abc import ABC, abstractmethod
from typing import Generic, TypeVar, Literal
from pydantic import BaseModel

P = TypeVar("P", bound=BaseModel)

class Body(BaseModel, Generic[P]):
    msg_id: int | None = None
    in_reply_to: int | None = None
    payload: P  # на самом деле flatten — в JSON прямо в body

class Message(BaseModel, Generic[P]):
    src: str
    dest: str
    body: Body[P]

    def into_reply(self, payload, next_id):
        new_msg = self.copy(deep=True)
        new_msg.src, new_msg.dest = self.dest, self.src
        new_msg.body.id = next_id[0]
        next_id[0] += 1
        new_msg.body.in_reply_to = self.body.msg_id
        new_msg.body.payload = payload
        return new_msg

class Node(ABC, Generic[P]):
    @classmethod
    @abstractmethod
    def from_init(cls, init): ...
    @abstractmethod
    def step(self, input, out): ...

def main_loop(node_cls, payload_cls):
    # читаем init, отвечаем init_ok, потом крутим цикл по stdin
    ...
```

И `echo/main.py`:

```python
class EchoPayload(BaseModel):
    type: Literal["echo", "echo_ok"]
    echo: str

class EchoNode(Node[EchoPayload]):
    def __init__(self): self.next_id = 1
    @classmethod
    def from_init(cls, init): return cls()
    def step(self, input, out):
        if input.body.payload.type == "echo":
            reply = input.into_reply(
                EchoPayload(type="echo_ok", echo=input.body.payload.echo),
                [self.next_id],
            )
            out.write(reply.json() + "\n")

if __name__ == "__main__":
    main_loop(EchoNode, EchoPayload)
```

Идея одна и та же. Разница в том, что Rust требует:
- Явно указывать владение и заимствование (`&`, `&mut`, `self` без `&`).
- Описывать enum как алгебраический тип, а не строкой типа.
- Делать всё статически проверяемым — нет «авось пройдёт в рантайме».
