# Monomorphization cost в Rust: разбор твоего примера

Твой код — отличная демонстрация того, что `log_outer::<T>` **работает** для всех этих типов, но за кулисами создаёт **отдельный инстанс на каждый `T`**. Разберём, сколько именно и почему это дёшево.

## Сколько инстансов порождает твой `main`

| # | Вызов | `T` | Инстанс |
|---|-------|-----|---------|
| 1 | `log_outer("&str")` | `&str` | `log_outer::<&str>` |
| 2 | `log_outer(String::from(...))` | `String` | `log_outer::<String>` |
| 3 | `log_outer(&s)` | `&String` | `log_outer::<&String>` |
| 4 | `log_outer(s)` (Box) | `Box<str>` | `log_outer::<Box<str>>` |
| 5 | `log_outer(s)` (Cow) | `Cow<'_, str>` | `log_outer::<Cow<str>>` |
| 6 | `log_outer(s)` (Rc) | `Rc<str>` | `log_outer::<Rc<str>>` |
| 7 | `log_outer(s)` (Arc) | `Arc<str>` | `log_outer::<Arc<str>>` |
| 8 | `log_outer(s.as_ref())` | `&str` | **тот же** `log_outer::<&str>` (уже есть) |

Итого **7 уникальных инстансов** `log_outer` (8-й вызов переиспользует `&str`).

> Заметь: в вызове 8 ты написал `log_outer(s.as_ref())`, а не `log_outer(s)`. Это принципиально — если бы написал `log_outer(s)`, то `T = MyString` и появился бы **8-й** инстанс `log_outer::<MyString>`. Ты сам того не заметив, применил приём из конспекта: привёл к `&str` на call-site.

## Почему это дёшево

Ключ — в том, что **вся работа** живёт в `log_inner`, а `log_outer` — тонкая:

```rust
fn log_inner(s: &str) { println!("{}", s); }   // ОДНА копия на весь бинарник

fn log_outer<T: AsRef<str>>(s: T) {
    log_inner(s.as_ref());                      // только as_ref + call
}
```

Что делает каждый инстанс `log_outer::<T>`:

```rust
// Псевдо-код кодогенерации:
fn log_outer_str(s: &str)          { log_inner(s) }              // as_ref = identity
fn log_outer_string(s: String)     { log_inner(s.as_str()) }
fn log_outer_ref_string(s: &String){ log_inner(s.as_str()) }
fn log_outer_box_str(s: Box<str>)  { log_inner(&*s) }
fn log_outer_cow(s: Cow<str>)      { log_inner(s.as_ref()) }
fn log_outer_rc_str(s: Rc<str>)    { log_inner(&*s) }
fn log_outer_arc_str(s: Arc<str>)  { log_inner(&*s) }
```

Каждый — буквально `as_ref()` (часто no-op) + `call log_inner`. LLVM обычно **инлайнит** их в call-site, и в бинарнике остаётся **один** `log_inner` + несколько `call`-сайтов. Именно поэтому твой пример — образец «правильной» мономорфизации: **толстая логика не размножается**.

## Что было бы плохо

Сравни с «наивным» вариантом, где вся логика внутри generic:

```rust
// ❌ ПЛОХО: тело дублируется на каждый T
fn log_outer_bad<T: AsRef<str>>(s: T) {
    let s = s.as_ref();
    // ... 50 строк форматирования, парсинга, ветвлений ...
    println!("{}", s);   // и всё это — в каждой копии
}
// → 7 копий всех 50 строк, по одной на каждый T
```

Или хуже — generic, который **внутри** вызывает другие generic:

```rust
// ❌ ОЧЕНЬ ПЛОХО: мономорфизация «протекает» вглубь
fn process<T: AsRef<str>>(s: T) -> Vec<u8> {
    s.as_ref()
        .chars()               // generic iterator
        .map(|c| c as u8)      // generic adapter
        .filter(|&b| b > 0)    // generic adapter
        .collect()             // generic collect
}
// → каждый T размножает всю цепочку итераторов + collect
```

Здесь на каждый `T` компилятор строит **свою** цепочку `Chars<Map<Filter<...>>>` и **свою** реализацию `collect`. На сотнях типов это уже заметно и по размеру, и по времени компиляции.

## Приёмы из конспекта — на твоём примере

### 1. Тонкая generic-обёртка + толстая non-generic реализация ✅

Твой `log_outer` → `log_inner` — ровно это. Правило: **generic должен только привести тип к общему виду и вызвать негенерик-функцию.**

### 2. Не делать generic там, где достаточно `&str` / `&[u8]`

```rust
// Вместо:
fn log_outer<T: AsRef<str>>(s: T) { ... }

// Часто достаточно:
fn log_outer(s: &str) { ... }
// и вызывающий пишет: log_outer(&s), log_outer(s.as_str()), log_outer(&*boxed), ...
```

В твоём случае generic оправдан, потому что ты **демонстрируешь** разные `T`. В реальном коде, если вызывающему несложно сделать `&s`, лучше принять `&str` — и мономорфизации не будет вообще (одна функция).

### 3. `impl Trait` внутри, а не на API-границе

Твой второй пример из конспекта:

```rust
pub fn process_data(data: &[u8]) -> Vec<u8> {   // негенерик на границе
    let iter = data.iter().map(|&x| x * 2);
    collect_with_optimization(iter)              // generic внутри
}

fn collect_with_optimization(iter: impl Iterator<Item = u8>) -> Vec<u8> {
    iter.collect()                                // impl Trait — только здесь
}
```

`collect_with_optimization` — generic, но она **внутренняя** и вызывается из **одного** места с **одним** типом итератора. Значит, инстанс ровно один. Наружу generic не «протёк».

## Как измерить мономорфизацию

Лучший способ убедиться, что инстансы действительно тонкие:

```bash
# Сколько строк LLVM IR генерирует каждый crate (ищет раздувание от generic)
cargo install cargo-llvm-lines
cargo llvm-lines --release | head -20

# Размер функций в бинарнике — видно, размножился ли log_outer
cargo install cargo-bloat
cargo bloat --release --crates

# Символы в бинарнике (nm)
nm target/release/o_087 | grep log_outer
```

Если `log_outer` в `nm` встречается 7 раз, но каждая — пара инструкций, а `log_inner` — один, всё хорошо. Если `log_outer::<T>` весит килобайты — значит, логика «протекла» внутрь generic.

## Практические правила

| Ситуация | Что делать |
|----------|------------|
| Публичный API, вызывающий имеет конкретный тип | Принимай `&str` / `&[u8]` / `&dyn Trait` |
| Нужна гибкость `T` (возврат `T`, `const`-generic, ассоциированные типы) | Generic, но тонкий — логика в негенерик-функции |
| Логика тяжёлая, типов много | `&dyn Trait` вместо generic |
| Generic внутри функции, не на границе | `impl Trait` — ок, инстанс один |
| Профилирование показывает раздувание | `cargo llvm-lines`, `cargo bloat` |

## Итог

- Твой `main` порождает **7 инстансов** `log_outer::<T>` (8-й вызов переиспользует `&str`).
- Это **дёшево**, потому что `log_outer` — тонкая обёртка, а вся логика — в единственной `log_inner`.
- Приём из конспекта работает: **тонкий generic → толстый негенерик**.
- Опасность — когда generic «протекает» вглубь (цепочки итераторов, вложенные generic), тогда на каждый `T` размножается вся внутренность.
- Лекарства: `&str`/`&[u8]` на API-границе, `impl Trait` только внутри, `&dyn Trait` при большом числе типов, измерение через `cargo llvm-lines` / `cargo bloat`.
