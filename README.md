# server_prac

A hand-rolled HTTP/1.1 server written from scratch on `std::net` — no frameworks, no dependencies, no async runtime. Built while working through the final project of [The Rust Book](https://doc.rust-lang.org/book/ch20-00-final-project-a-web-server.html).

It serves one hardcoded page at `/` and a 404 page for everything else, from a fixed pool of worker threads.

---

## Status

| Component | File | State |
|---|---|---|
| HTTP request handling | `src/main.rs` | ✅ Complete |
| Thread pool | `src/lib.rs` | ❌ Two defects — see below |

> ⚠️ **The package does not build.** `src/lib.rs` is missing a semicolon, so `cargo build` and `cargo run` both fail. Once that's fixed, a second bug prevents any response from being sent.

### Current blockers

**1. Syntax error — `lib.rs:38`**

```rust
Err(_) => {
    println!("Worker {id} has disconnected from job, shutting down!")   // ← missing ;
    break;
}
```

```
error: expected `;`, found keyword `break`
  --> src\lib.rs:38:90
```

`println!` is an expression, not a statement, so it needs a `;` before `break`. This alone stops compilation.

**2. The job is never executed — `lib.rs:34-36`**

```rust
Ok(job) => {
    println!("Worker {id} aquired a job, now executing!");
}   // ← job is dropped here without being called
```

The worker receives the job, prints a message, and then drops it. Since `job` is the boxed closure that *contains* `handle_connection` and the `TcpStream`, dropping it closes the connection without writing a response. Every client would hang or receive an empty reply.

The fix is one line inside the `Ok` arm:

```rust
Ok(job) => {
    println!("Worker {id} aquired a job, now executing!");
    job();          // ← run the closure
}
```

`execute`'s `self.sender.as_ref().unwrap().send(job).unwrap()` is fine, and the `Some((sender))` on line 71 only produces an `unused_parens` warning.

---

## What it can do today

Nothing yet — it doesn't compile. What `main.rs` implements, once `lib.rs` is fixed:

| Capability | Supported |
|---|---|
| Bind and listen on `127.0.0.1:7878` | ✅ (code complete) |
| Accept connections | ✅ (code complete) |
| Dispatch each connection to a worker thread | ⚠️ Dispatched, but never run |
| Read the HTTP request line | ✅ (code complete) |
| Route `GET / HTTP/1.1` → `return.html` with `200 OK` | ✅ (code complete) |
| Route anything else → `wrong.html` with `404 NOT FOUND` | ✅ (code complete) |
| Send a `Content-Length` header and body | ✅ (code complete) |
| Handle more than one client at a time | ⚠️ Blocked by the defects above |
| Serve arbitrary paths / files | ❌ |
| Serve `style.css` or `index.js` | ❌ (they get the 404 page) |
| Parse request headers or bodies | ❌ |
| Support `POST`, query strings, keep-alive | ❌ |
| Shut down gracefully | ⚠️ `Drop` impl exists but is unreachable |
| Survive an I/O error | ❌ (every I/O call is `unwrap()`) |

---

## How it works

```
TcpListener::bind("127.0.0.1:7878")
        │
        │  listener.incoming()  — blocks until a client connects
        ▼
    TcpStream
        │  pool.execute(|| handle_connection(stream))
        ▼
   mpsc channel ──► one of 5 worker threads
        │
        ▼
  handle_connection
        │  BufReader::read_line
        ▼
  "GET / HTTP/1.1\r\n"
        │  starts_with("GET / HTTP/1.1") ?
        ├── yes ──► 200 OK   + return.html
        └── no  ──► 404      + wrong.html
        │
        ▼
  "HTTP/1.1 200 OK\r\nContent-Length: N\r\n\r\n<body>"
        │  stream.write_all
        ▼
      client
```

`main` owns the listener and the pool. Each accepted connection is moved into a closure and handed to `ThreadPool::execute`, which boxes it and sends it down a channel. A worker picks it up, runs it, and the connection closes when `handle_connection` returns.

Each worker prints to stdout when it picks up work:

```
Worker 0 aquired a job, now executing!
Worker 1 aquired a job, now executing!
```

### The response format

An HTTP/1.1 response is three parts, each separated by CRLF (`\r\n`), with a blank line between headers and body:

```
HTTP/1.1 200 OK\r\n
Content-Length: 312\r\n
\r\n
<!DOCTYPE html>...
```

`Content-Length` is the **byte** length of the body — `page.len()` on a `String`, which is why it's correct even for non-ASCII text. Without it, the client has to wait for the connection to close to know the body ended.

---

## Routes

| Request line | Status | Body |
|---|---|---|
| `GET / HTTP/1.1` | `200 OK` | `return.html` |
| *anything else* | `404 NOT FOUND` | `wrong.html` |

The match is a literal `starts_with` on the request line, so only the bare root path works. `GET /index.html`, `GET /?page=1`, and `GET /favicon.ico` all fall through to the 404 page.

---

## Getting started

```bash
cargo build     # currently fails — see Current blockers
cargo run       # then open http://127.0.0.1:7878
```

The HTML files reference `style.css` and `index.js`, which don't exist in this project — and the server has no static-file handling anyway, so they'll 404 and the page will render unstyled.

---

## Project layout

```
server_prac/
├── Cargo.toml
├── Cargo.lock
├── return.html          # served on 200
├── wrong.html           # served on 404
└── src/
    ├── main.rs          # listener, routing, response building
    └── lib.rs           # ThreadPool, Worker, Job
```

---

## Thread pool

`lib.rs` now implements the full design from the Book:

- `type Job = Box<dyn FnOnce() + Send + 'static>` — the unit of work
- `ThreadPool::new(size)` asserts `size > 0`, creates a `mpsc` channel, wraps the `Receiver` in `Arc<Mutex<_>>`, and spawns `size` workers sharing it
- Each worker runs `loop { recv() }` and breaks when the channel closes
- `ThreadPool::execute(f)` boxes the closure and sends it down the channel
- `Drop for ThreadPool` takes the `Sender` out of its `Option`, dropping it to close the channel, then joins every worker

The `Arc<Mutex<Receiver>>` is what makes a multi-consumer channel work: `mpsc::Receiver` can't be cloned, so the workers share one behind a mutex. The guard is a temporary in `receiver.lock().unwrap().recv()`, so it's released before the job runs — jobs execute concurrently rather than serially.

Storing the sender as `Option<Sender<Job>>` is the trick that makes shutdown possible: `Drop` can't move a field out of `&mut self`, but it *can* `take()` an `Option` and drop the value inside.

### Still to fix

1. Add the missing `;` at `lib.rs:38`.
2. Call `job()` inside the `Ok` arm at `lib.rs:34`.

---

## Known limitations

- **Doesn't compile.** Missing `;` at `lib.rs:38`.
- **Jobs are never executed.** `job()` is never called, so no response is ever written.
- **`unwrap()` on every I/O call.** A refused connection or a broken pipe panics the process instead of returning an error.
- **Graceful shutdown is unreachable.** `Drop` only runs when `main` returns, but `main` loops forever on `listener.incoming()`. Ctrl+C kills the process without unwinding, so the join logic never executes in practice.
- **Exact-path routing only.** No path parsing, no query strings, no `HEAD`.
- **No `Connection` header.** HTTP/1.1 defaults to keep-alive, but the socket is closed after one response. It works because the client sees EOF, but it's not what the spec expects.
- **Hardcoded host, port, and pool size.** All three are literals in `main`.
- **No content types.** `Content-Type` is never sent, so browsers sniff the body.
- **No tests.**

---

## Concepts this project exercises

- `TcpListener` / `TcpStream` and blocking accept loops
- `BufReader` + `Read::read_line` for incremental parsing
- The HTTP/1.1 message format (status line, headers, CRLF, body)
- `Write::write_all` vs `write`
- Byte length vs character count (`String::len`)
- `std::fs::read_to_string`
- `thread::spawn` and `JoinHandle`
- `mpsc` channels as a work queue
- `Arc<Mutex<T>>` for sharing a receiver across threads
- `Box<dyn FnOnce() + Send + 'static>` as a type-erased job
- Closures that capture and move ownership (`move ||`)
- `Drop` and `Option::take()` for controlled shutdown
- `Vec::drain` and joining threads

---

## License

No license file. Add one (MIT or Apache-2.0) if you intend to share this.
