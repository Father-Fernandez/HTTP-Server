# server_prac

A hand-rolled HTTP/1.1 server written from scratch on `std::net` — no frameworks, no dependencies, no async runtime. Built while working through the final project of [The Rust Book](https://doc.rust-lang.org/book/ch20-00-final-project-a-web-server.html).

---

## Status

| Component | File | State |
|---|---|---|
| Single-threaded HTTP server | `src/main.rs` | ✅ Complete and working |
| Thread pool | `src/lib.rs` | 🚧 Scaffold only — **does not compile yet** |

> ⚠️ **The package does not build right now.** `src/lib.rs` has an unfinished thread pool with a `use of moved value` error (`E0382`), and because the binary target implicitly depends on the library, `cargo build` and `cargo run` both fail. The server code in `main.rs` is correct — it ran before `lib.rs` was added. See [Thread pool](#thread-pool-in-progress).

---

## What it can do today

| Capability | Supported |
|---|---|
| Bind and listen on `127.0.0.1:7878` | ✅ |
| Accept connections sequentially | ✅ |
| Read the HTTP request line | ✅ |
| Route `GET / HTTP/1.1` → `return.html` with `200 OK` | ✅ |
| Route anything else → `wrong.html` with `404 NOT FOUND` | ✅ |
| Send a `Content-Length` header and body | ✅ |
| Handle more than one client at a time | ❌ |
| Serve arbitrary paths / files | ❌ |
| Serve `style.css` or `index.js` | ❌ (they get the 404 page) |
| Parse request headers or bodies | ❌ |
| Support `POST`, query strings, keep-alive | ❌ |
| Shut down gracefully | ❌ |

---

## How it works

```
TcpListener::bind("127.0.0.1:7878")
        │
        │  listener.incoming()  — blocks until a client connects
        ▼
   TcpStream
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

`main` binds the listener and loops over `listener.incoming()`, handing each `TcpStream` to `handle_connection`, which does all the work. The connection is dropped — and therefore closed — at the end of each iteration.

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
cargo build     # currently fails — see Status
cargo run       # then open http://127.0.0.1:7878
```

Once the thread pool is fixed (or `src/lib.rs` is temporarily emptied), the server starts and serves `return.html` at the root.

The HTML files reference `style.css` and `index.js`, which don't exist in this project — and the server has no static-file handling anyway, so they'll 404.

---

## Project layout

```
server_prac/
├── Cargo.toml
├── Cargo.lock
├── return.html          # served on 200
├── wrong.html           # served on 404
└── src/
    ├── main.rs          # listener + request handling
    └── lib.rs           # ThreadPool (work in progress)
```

---

## Thread pool (in progress)

`lib.rs` sketches a fixed-size worker pool: a `mpsc` channel of jobs, N worker threads, and an `execute` method that hands a closure to the pool.

What exists:

- `ThreadPool { workers, sender }` and `Worker { id, thread }`
- `ThreadPool::new(size)` which asserts `size > 0` and spawns workers
- `execute<F: FnOnce() + Send + 'static>` — signature only, empty body

What's blocking the build:

```rust
let (sender, reciever) = mpsc::channel();

for id in 0..size {
    workers.push(Worker::new(id, reciever))   // ✗ moved on the first iteration
}
```

`Receiver` is not `Copy`, so passing it by value into `Worker::new` moves it — the second loop iteration has nothing left to pass. The Book's solution is `Arc<Mutex<Receiver<Job>>>`, with each worker locking the receiver in a `loop { recv() }`. `Job` also needs to become `Box<dyn FnOnce() + Send + 'static>` rather than the current empty struct.

---

## Known limitations

- **Single-threaded.** One client that hangs up mid-request blocks every other client until it times out.
- **`unwrap()` on every I/O call.** A refused connection or a broken pipe panics the whole server.
- **Exact-path routing only.** No path parsing, no query strings, no `HEAD`.
- **No `Connection` header.** HTTP/1.1 defaults to keep-alive, but the socket is closed after one response. It works because the client sees EOF, but it's not what the spec expects.
- **Hardcoded host and port.** No configuration.
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
- Threads, `mpsc` channels, `Arc<Mutex<T>>` (work in progress)

---

## Honesty

The only A.I generated component is the Readme, everything else was written by hand
