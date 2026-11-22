use std::net::{TcpListener, TcpStream};
use std::io::{BufReader, prelude::*};
use std::fs;


fn main() {
    // set host connection
    let listener =
        TcpListener::bind("127.0.0.1:7878").unwrap();

    //parse through each request made on the socket
    for stream in listener.incoming(){
        let stream = stream.unwrap();

        handle_connection(stream);
    }
}


//take the incoming "streams" and formats them into strings`
//When writing a requestion format should be:

//HTTP-Version Status-Code Reason-Phrase CRLF '\r\n' (Carriage Return and Line Feed)
//Example HTTP/1.1 200 OK\r\n\r\n
//Headers CRLF
//Message - Body
fn handle_connection(mut stream: TcpStream) {
    let mut buffer = BufReader::new(&stream);
    let mut get = String::new();
    buffer.read_line(&mut get).unwrap();

    let (status_line, file_name) =

        if get.starts_with("GET / HTTP/1.1"){
            ("HTTP/1.1 200 OK", "return.html")
        } else {
            ("HTTP/1.1 404 NOT FOUND", "wrong.html")
        };

    let page = fs::read_to_string(file_name).unwrap();

    let response = format!(
        "{}\r\nContent-Length: {}\r\n\r\n{}",
        status_line, page.len(), page
    );

    stream.write_all(response.as_bytes()).unwrap();

    //putting respose as "HTTP/1.1 200 OK\r\n\r\n" gets the page to load without error

    //printing data from a raw request
    // println!(
    //     "Request: {}", String::from_utf8_lossy(&buffer[..102])
    //     //the way to format them
    // )
}
