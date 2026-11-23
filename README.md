# server_prac

A hand-rolled HTTP/1.1 server written from scratch on `std::net` — no frameworks, no dependencies, no async runtime. Built while working through the final project of [The Rust Book](https://doc.rust-lang.org/book/ch20-00-final-project-a-web-server.html).

It serves one hardcoded page at `/` and a 404 page for everything else, from a fixed pool of five worker threads.

---

## Status

| Component | File | State |
|---|---|---|
| HTTP request handling | `src/main.rs` | ✅ Complete |
| Thread pool | `src/lib.rs` | ✅ Complete — workers loop, jobs execute, shutdown joins cleanly |

Builds and runs. `cargo build` emits one cosmetic warning (`unused_parens` on `Some((sender))`).

---

## What it can do today

| Capability | Supported |
|---|---|
| Bind and listen on `127.0.0.1:7878` | ✅ |
| Accept connections continuously | ✅ |
| Dispatch each connection to a worker thread | ✅ |
| Handle 5 requests concurrently | ✅ |
| Queue additional requests until a worker frees up | ✅ |
| Read the HTTP request line | ✅ |
| Route `GET / HTTP/1.1` → `return.html` with `200 OK` | ✅ |
| Route anything else → `wrong.html` with `404 NOT FOUND` | ✅ |
| Send a `Content-Length` header and body | ✅ |
| Serve arbitrary paths / files | ❌ |
| Serve `style.css` or `index.js` | ❌ (they get the 404 page) |
| Parse request headers or bodies | ❌ |
| Support `POST`, query strings, keep-alive | ❌ |
| Shut down gracefully | ⚠️ `Drop` impl is correct but unreachable in practice |
| Survive an I/O error | ❌ (every I/O call is `unwrap()`) |

---

## Verified behaviour

Measured against the compiled binary:

| Test | Result |
|---|---|
| 8 sequential requests | 8 × `200 OK`, 460 bytes each |
| Request to `/nope` | `404 NOT FOUND` + `wrong.html` |
| 30 concurrent requests | 30 / 30 `200 OK` |
| 50 sequential requests | 50 / 50 `200 OK` |
| Process after all of the above | Still running, empty stderr |

The pool is 5 workers, so the 30 concurrent requests are accepted by the OS, queued in the channel, and drained 5 at a time. Nothing is dropped or refused.

Worker activity is visible on stdout:

```
Worker 0 aquired a job, now executing!
Worker 1 aquired a job, now executing!
Worker 2 aquired a job, now executing!
Worker 3 aquired a job, now executing!
Worker 4 aquired a job, now executing!
Worker 0 aquired a job, now executing!     ← worker 0 takes a second job
```

That repeat is the proof the loop works: each worker handles many jobs over its lifetime instead of exiting after one.

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
cargo run
```

Then open <http://127.0.0.1:7878>, or from the command line:

```bash
curl -i http://127.0.0.1:7878/          # 200 + return.html
curl -i http://127.0.0.1:7878/missing   # 404 + wrong.html
```

Stop it with `Ctrl+C`.

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

`lib.rs` implements a fixed-size pool:

- `type Job = Box<dyn FnOnce() + Send + 'static>` — the unit of work
- `ThreadPool::new(size)` asserts `size > 0`, creates a `mpsc` channel, wraps the `Receiver` in `Arc<Mutex<_>>`, and spawns `size` workers sharing it
- Each worker runs `loop { recv() }` and breaks when the channel closes
- `ThreadPool::execute(f)` boxes the closure and sends it down the channel
- `Drop for ThreadPool` takes the `Sender` out of its `Option`, dropping it to close the channel, then joins every worker

### Why it's shaped this way

**`Arc<Mutex<Receiver>>`** — `mpsc::Receiver` can't be cloned, so all five workers must share one. The mutex makes concurrent access safe.

**The lock is released before the job runs.** `receiver.lock().unwrap().recv()` is a single expression, so the temporary `MutexGuard` is dropped at the semicolon — before `job()` is called. If the guard lived across `job()`, every request would be serialized and the pool would be pointless. This is the subtlest line in the file.

**`Option<Sender<Job>>`** — `Drop::drop` receives `&mut self` and can't move a field out of it, but it *can* `take()` an `Option` and drop the contents. Dropping the last `Sender` closes the channel, which makes every worker's `recv()` return `Err` and break its loop. That's the shutdown signal.

**`Vec::drain(..)`** in `Drop` empties the vector while iterating, so each `Worker` is moved out and joined.

---

## Known limitations

- **Graceful shutdown is unreachable.** `Drop for ThreadPool` only runs when `main` returns, but `main` loops forever on `listener.incoming()`. `Ctrl+C` terminates the process without unwinding, so the join logic never actually executes. It's correct code that needs a shutdown trigger (signal handler, request counter) to be useful.
- **`unwrap()` on every I/O call.** A refused connection or a broken pipe panics the process instead of returning an error.
- **Exact-path routing only.** No path parsing, no query strings, no `HEAD`.
- **No `Connection` header.** HTTP/1.1 defaults to keep-alive, but the socket is closed after one response. It works because the client sees EOF, but it's not what the spec expects.
- **Hardcoded host, port, and pool size.** All three are literals in `main`.
- **No content types.** `Content-Type` is never sent, so browsers sniff the body.
- **Pool size is not configurable at runtime.** `ThreadPool::new(5)` is a magic number.
- **One cosmetic warning.** `Some((sender))` on `lib.rs:73` has redundant parentheses.
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
