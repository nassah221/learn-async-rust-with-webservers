use std::io::{self, Read, Write};
use std::net::TcpStream;

enum ConnectionState {
    Read {
        request: [u8; 1024],
        read: usize,
    },
    Write {
        response: &'static [u8],
        written: usize,
    },
    Flush,
}

fn main() {
    let listener = std::net::TcpListener::bind("localhost:3000").unwrap();
    listener.set_nonblocking(true).unwrap();

    let mut connections = Vec::new();

    loop {
        match listener.accept() {
            Ok((conn, _)) => {
                conn.set_nonblocking(true).unwrap();

                let state = ConnectionState::Read {
                    request: [0u8; 1024],
                    read: 0,
                };

                connections.push((conn, state));
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => panic!("{e}"),
        }

        let mut completed = Vec::new();

        'next: for (i, (conn, state)) in connections.iter_mut().enumerate() {
            if let ConnectionState::Read { request, read } = state {
                loop {
                    match conn.read(&mut request[*read..]) {
                        Ok(0) => {
                            println!("client disconnected unexpectedly");
                            completed.push(i);
                            continue 'next;
                        }
                        Ok(n) => {
                            *read += n;
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            continue 'next;
                        }
                        Err(e) => panic!("{e}"),
                    }
                    if request.get(*read - 4..*read) == Some(b"\r\n\r\n") {
                        break;
                    }
                }
                let request = String::from_utf8_lossy(&request[..*read]);
                println!("{request}");

                let response = concat!(
                    "HTTP/1.1 200 OK\r\n",
                    "Content-Length: 12\n",
                    "Connection: close\r\n\r\n",
                    "Hello world!"
                );

                *state = ConnectionState::Write {
                    response: response.as_bytes(),
                    written: 0,
                };
            }
            if let ConnectionState::Write { response, written } = state {
                loop {
                    match conn.write(&response[*written..]) {
                        Ok(0) => {
                            println!("client disconnected unexpectedly");
                            completed.push(i);
                            continue 'next;
                        }
                        Ok(n) => {
                            *written += n;
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue 'next,
                        Err(e) => panic!("{e}"),
                    }
                    if *written == response.len() {
                        break;
                    }
                }
                *state = ConnectionState::Flush;
            }
            if let ConnectionState::Flush = state {
                match conn.flush() {
                    Ok(_) => {
                        completed.push(i); // 👈
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        // not ready yet, move on to the next connection
                        continue 'next;
                    }
                    Err(e) => panic!("{e}"),
                }
            }
        }

        for i in completed.into_iter().rev() {
            connections.remove(i);
        }
    }
}

fn handle_connection(mut conn: TcpStream) -> std::io::Result<()> {
    let mut request = [0u8; 1024];
    let mut read = 0;

    loop {
        let num_bytes = conn.read(&mut request[read..])?;
        if num_bytes == 0 {
            println!("client disconnected unexpectedly");
            break;
        }

        read += num_bytes;
        // return if we have reached the end of the request
        if request.get(read - 4..read) == Some(b"\r\n\r\n") {
            break;
        }
    }

    let request = String::from_utf8_lossy(&request[..read]);
    println!("{request}");

    let response = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Content-Length: 12\n",
        "Connection: close\r\n\r\n",
        "Hello world!"
    );

    let mut written = 0;

    loop {
        let num_bytes = conn.write(response[written..].as_bytes())?;
        if num_bytes == 0 {
            println!("client disconnected unexpectedly");
            return Ok(());
        }
        written += num_bytes;

        if written == response.len() {
            break;
        }
    }

    conn.flush()?;

    Ok(())
}
