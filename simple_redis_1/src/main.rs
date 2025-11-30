use std::{collections::HashMap, sync::{Arc, Mutex}};

use bytes::Bytes;
use mini_redis::{Connection, Frame};
use tokio::net::{
    TcpListener, TcpStream
};


type Db = Arc<Mutex<HashMap<String, Bytes>>>;

#[tokio::main]
async fn main()  {

    let listner = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    let db = Arc::new(Mutex::new(HashMap::<String, Bytes>::new()));


    loop {
        let (socket, _) = listner.accept().await.unwrap();

        let db = db.clone();
        // process(socket).await;
        tokio::spawn(async move {
            process(socket, db).await;
        });
        
    }

}

async fn process_v1(socket: TcpStream) {
    // `Connection` 对于 redis 的读写进行了抽象封装
    // 因此我们读到的是一个一个数据帧frame(数据帧 = redis命令 + 数据)，而不是字节流
    // `Connection` 是在 mini-redis 中定义
    let mut connection = Connection::new(socket);

    if let Some(frame) = connection.read_frame().await.unwrap() {
        println!("Got frame: {:?}", frame);
        // 回复一个错误
        let response = Frame::Error("unimplemented".to_string());
        connection.write_frame(&response).await.unwrap();
    }
}

async fn process(socket: TcpStream, db: Db) {
    use mini_redis::Command::{self, Get, Set};


    let mut connection = Connection::new(socket);

    while let Some(frame) = connection.read_frame().await.unwrap() {
        let response = match Command::from_frame(frame).unwrap() {
            Set(cmd) => {
                let key = cmd.key().to_string();
                let value = cmd.value().clone();
                let mut db = db.lock().unwrap();
                db.insert(key, value);
                Frame::Simple("OK".to_string())
            }

            Get(cmd) => {
                let key = cmd.key().to_string();
                let db = db.lock().unwrap();
                if let Some(value) = db.get(&key) {
                    Frame::Bulk(value.clone().into())
                } else {
                    Frame::Null
                }
            }

            cmd => panic!("unimplemented {:?}", cmd),
        };

        connection.write_frame(&response).await.unwrap();
    }
}
