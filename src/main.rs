use std::{
    fs,
    io::{BufReader, prelude::*},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread,
    time::Duration,
};

use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::{ServerConnection, pki_types::pem::PemObject};

use web_server::ThreadPool;

fn handle_connection(mut stream: TcpStream, config: Arc<rustls::ServerConfig>) {
    let mut server_connection = match ServerConnection::new(config) {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("Failed to create server connection: {}", e);
            return;
        }
    };

    let tls_stream = rustls::Stream::new(&mut server_connection, &mut stream);
    let mut buf_reader = BufReader::new(tls_stream);

    let mut request = Vec::new();

    match buf_reader.read_until(b'\n', &mut request) {
        Ok(0) => {
            return;
        }
        Ok(_) => {
            while !request.ends_with(b"\r\n\r\n") {
                let mut line = Vec::new();
                if let Ok(0) = buf_reader.read_until(b'\n', &mut line) {
                    break;
                }
                request.extend_from_slice(&line);
            }
        }
        Err(e) => {
            eprintln!("Error reading from TLS stream: {e}");
            return;
        }
    };

    let request_line = String::from_utf8_lossy(&request);
    let home = "GET / HTTP/1.1".to_string();
    let sleep = "GET /sleep HTTP/1.1".to_string();
    let (status_line, filename) = if request_line.starts_with(&home) {
        ("HTTP/1.1 200 OK", "hello.html")
    } else if request_line.starts_with(&sleep) {
        thread::sleep(Duration::from_secs(5));
        ("HTTP/1.1 200 OK", "hello.html")
    } else {
        ("HTTP/1.1 404 NOT FOUND", "404.html")
    };

    let contents = fs::read_to_string(filename).unwrap();
    let length = contents.len();

    let response = format!(
        "{}\r\nContent-Length: {}\r\n\r\n{}",
        status_line, length, contents
    );

    let mut tls_stream = buf_reader.into_inner();

    if let Err(e) = tls_stream.write_all(response.as_bytes()) {
        eprintln!("Error writing to TLS stream: {}", e);
    }
    if let Err(e) = tls_stream.flush() {
        eprintln!("Error flushing TLS stream: {}", e);
    }
}

fn main() {
    let certs = CertificateDer::pem_file_iter("cert.pem")
        .unwrap()
        .map(|cert| cert.unwrap())
        .collect();
    let private_key = PrivateKeyDer::from_pem_file("key.pem").unwrap();
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, private_key)
        .unwrap();

    let listener = TcpListener::bind("0.0.0.0:7878").unwrap();
    let arc_config = Arc::new(config);

    let pool = ThreadPool::new(12);

    for stream in listener.incoming() {
        match stream {
            Ok(tcp_stream) => {
                let config_clone = Arc::clone(&arc_config);
                pool.execute(|| handle_connection(tcp_stream, config_clone));
            }
            Err(e) => {
                eprintln!("Connection failed: {}", e);
            }
        }
    }
}
