use std::{
    fs,
    io::{BufReader, prelude::*},
    net::{TcpListener, TcpStream},
};
fn main() {
    println!("Hello, There!");

    let listener = TcpListener::bind("127.0.0.1:12580").unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        // println!("Connection established!");
        handle_connection(stream);
    }
}

fn handle_connection(mut stream: TcpStream) {
    let buf_reader = BufReader::new(&mut stream);
    let request_line = buf_reader.lines().next().unwrap().unwrap();

    let (status_line, filename) = if request_line == "GET / HTTP/1.1" {
        ("HTTP/1.1 200 OK", "hello.html")
    } else {
        ("HTTP/1.1 404 NOT FOUND", "404.html")
    };

    let content = fs::read_to_string(filename).unwrap();
    let length = content.len();

    let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n\n{content}");

    stream.write_all(response.as_bytes()).unwrap();
}

fn handle_connection_v1(mut stream: TcpStream) {
    let buf_reader = BufReader::new(&mut stream);
    let http_request: Vec<_> = buf_reader
        // - lines 是 std::io::BufRead trait 的方法，不是 BufReader 的固有方法
        // use std::io::prelude::*; 会把 BufRead （以及 Read 、 Write 等）导入
        .lines()
        .map(|result| result.unwrap())
        // like js filter 但是会停止迭代 - 连续拿取“从开头开始”满足条件的元素；第一次条件为假就停止，后面的元素不再遍历
        .take_while(|line| !line.is_empty())
        .collect();

    println!("Request: {:#?}", http_request);

    let content = fs::read_to_string("hello.html").unwrap();
    let length = content.len();

    let status_line = "HTTP/1.1 200 OK\r\n\r\n";
    let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n\n{content}");

    stream.write_all(response.as_bytes()).unwrap();
}
