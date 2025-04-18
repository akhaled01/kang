# AUDIT QUESTIONS

## FUNCTIONAL

### How does an HTTP server works?

1. **Binds to IP/Port**: The server binds TCP sockets to configured IP addresses and ports.

```rust
let listener = TcpListener::bind(addr)?;
listener.set_nonblocking(true)?;
```

2. **Performs I/O multiplexing**: Kang uses epoll (Linux) or kqueue (macOS) to efficiently monitor multiple file descriptors.

```rust
#[cfg(target_os = "linux")]
let global_fd = unsafe { epoll_create1(0) };
```

3. **Accepts client connections**: When the listener socket is ready, new connections are accepted.

```rust
match self.listener.accept() {
    Ok((stream, addr)) => {
        stream.set_nonblocking(true)?;
        // Register the new connection...
    }
}
```

4. **Reads and parses HTTP requests**: The server reads data and attempts to parse valid HTTP requests.

```rust
match stream.read(&mut temp_buf) {
    // ...parse what we have so far
    match Request::parse(&buffer) {
        Ok(request) => return Ok(request),
    }
}
```

5. **Routes requests**: Kang uses a multiplexer (Mux) to route requests to appropriate handlers.

```rust
let res = self.mux.handle(req);
```

6. **Generates and sends responses**: Finally, responses are formatted and sent back to clients.

```rust
match listener.send_bytes(res.to_bytes(), fd) {
    Ok(_) => { /* ... */ }
}
```

### Which function was used for I/O Multiplexing and how does it works?

Kang uses different multiplexing functions depending on the operating system:

- On Linux: *epoll_create1*, *epoll_ctl*, and *epoll_wait*:

```rust
let global_fd = unsafe { epoll_create1(0) };
// Register file descriptors
unsafe { epoll_ctl(global_fd, EPOLL_CTL_ADD, listener.get_id(), &mut event) }
// Wait for events
let nfds = unsafe { epoll_wait(global_fd, events.as_mut_ptr(), MAX_EVENTS as i32, -1) };
```

- On macOS: *kqueue* and *kevent*:

```rust
let global_fd = unsafe { kqueue() };
// Register file descriptors
unsafe { kevent(global_fd, &changes, 1, ptr::null_mut(), 0, ptr::null()) }
// Wait for events
let nfds = unsafe { kevent(global_fd, std::ptr::null(), 0, events.as_mut_ptr(), MAX_EVENTS as i32, std::ptr::null()) };
```

In both cases, the system call blocks until events occur on registered file descriptors, then returns with information about which descriptors are ready for I/O operations.

### Is the server using only one select (or equivalent) to read the client requests and write answers?

Yes, Kang uses exactly one multiplexing instance (*global_fd*) per server to handle all I/O operations. This is created in *Server::listen_and_serve()*:

```rust
#[cfg(target_os = "linux")]
let global_fd = unsafe { epoll_create1(0) };
#[cfg(target_os = "macos")]
let global_fd = unsafe { kqueue() };
```

All connections are registered with this single instance, and all I/O operations are driven by events from this multiplexer.

### Why is it important to use only one select and how was it achieved?

Using a single multiplexing instance is important for:

1. **Efficiency**: Avoids duplicate syscalls and context switches
2. **Resource management**: Prevents resource leaks from multiple instances
3. **Simplicity**: Centralizes event handling logic
4. **Scalability**: A single event loop can efficiently handle thousands of connections

Kang achieves this through:

* Creating one global descriptor for epoll/kqueue
* Registering all listeners and connections with this descriptor
* Using a single event loop to handle all events
* Sharing this descriptor across the server's components

### Read the code that goes from the select (or equivalent) to the read and write of a client, is there only one read or write per client per select (or equivalent)?

No, Kang implements multiple reads per event when possible:

When an EPOLLIN/EVFILT_READ event occurs, the server reads in a loop until WouldBlock:

```rust
loop {
    match stream.read(&mut temp_buf) {
        Ok(n) => {
            buffer.extend_from_slice(&temp_buf[..n]);
            // Continue reading...
        }
        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
            // No more data available without blocking
            break;
        }
        // Error handling...
    }
}
```

For writes, typically one write operation is performed per request:

```rust
match listener.send_bytes(res.to_bytes(), fd) {
    Ok(_) => {
        handled = true;
        let _ = listener.remove_connection(fd, global_fd);
        break;
    }
    // Error handling...
}
```

This approach optimizes I/O by reading all available data when notified of readiness.

### Are the return values for I/O functions checked properly?

Yes, Kang consistently checks return values for all I/O operations:

1. **System calls**: All epoll/kqueue calls are checked for errors.

```rust
if global_fd < 0 {
    return Err(io::Error::last_os_error());
}
```

2. **Connection acceptance**: Handled with proper error discrimination.

```rust
match self.listener.accept() {
    Ok((stream, addr)) => { /* Success case */ }
    Err(e) if e.kind() == io::ErrorKind::WouldBlock => { /* Non-blocking case */ }
    Err(e) => { /* Error case */ }
}
```

3. **Read/write operations**: Outcomes are properly checked with pattern matching.

```rust
match stream.read(&mut temp_buf) {
    Ok(0) => { /* Connection closed */ }
    Ok(n) => { /* Data received */ }
    Err(e) if e.kind() == io::ErrorKind::WouldBlock => { /* Would block */ }
    Err(e) => { /* Other errors */ }
}
```

### If an error is returned by the previous functions on a socket, is the client removed?

Yes, clients are removed when errors occur, except for WouldBlock errors:

```rust
Err(e) => {
    error!("Read error on fd={}: {}", fd, e);
    return Err(e);
}
```

And when handling the error higher up:

```rust
Err(e) => {
    error!("Failed to send response: {}", e);
    handled = true;
    let _ = listener.remove_connection(fd, global_fd);
    break;
}
```

The *remove_connection* method:

1. Deregisters the file descriptor from epoll/kqueue.
2. Removes the connection from the internal HashMap.
3. Logs the removal.

### Is writing and reading ALWAYS done through a select (or equivalent)?

Yes, all I/O operations in Kang are properly guarded by the multiplexing mechanism:

1. **Reading only occurs** after an EPOLLIN/EVFILT_READ event indicates data is available.
2. **Writing only occurs** after successfully processing a request, which itself only happened after an EPOLLIN event.
3. **Connection acceptance** only happens after receiving an event on the listener socket.

This design ensures that the server never blocks on I/O operations, maintaining its ability to handle many concurrent connections efficiently.

The code shows consistent discipline in using the event-driven model throughout all I/O operations, with no direct reads or writes outside the epoll/kqueue notification system.