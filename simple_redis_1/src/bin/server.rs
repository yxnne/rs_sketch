use bytes::{Buf, Bytes, BytesMut};
use mini_redis::frame::Error::Incomplete;
use mini_redis::{Frame, Result};
use std::{
    collections::HashMap,
    io::Cursor,
    sync::{Arc, Mutex},
};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

type Db = Arc<Mutex<HashMap<String, Bytes>>>;

#[tokio::main]
async fn main() {
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

struct Connection {
    stream: TcpStream,
    buffer: BytesMut,
    cursor: usize,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Connection {
        Connection {
            stream,
            buffer: BytesMut::with_capacity(4096),
            cursor: 0,
        }
    }
    /// 从连接读取一个帧
    ///
    /// 如果遇到EOF，则返回 None
    pub async fn read_frame(&mut self) -> Result<Option<Frame>> {
        // 具体实现
        loop {
            if let Some(frame) = self.parse_frame()? {
                return Ok(Some(frame));
            }

            // 确保缓冲区长度足够
            if self.buffer.len() == self.cursor {
                // 若不够，需要增加缓冲区长度
                self.buffer.resize(self.cursor * 2, 0);
            }

            // 从游标位置开始将数据读入缓冲区
            let n = self.stream.read(&mut self.buffer[self.cursor..]).await?;

            if 0 == n {
                if self.cursor == 0 {
                    return Ok(None);
                } else {
                    return Err("connection reset by peer".into());
                }
            } else {
                // 更新游标位置
                self.cursor += n;
            }
        }
    }

    /// 将帧写入到连接中
    pub async fn write_frame(&mut self, frame: &Frame) -> io::Result<()> {
        match frame {
            Frame::Simple(val) => {
                self.stream.write_u8(b'+').await?;
                self.stream.write_all(val.as_bytes()).await?;
                self.stream.write_all(b"\r\n").await?;
            }
            Frame::Error(val) => {
                self.stream.write_u8(b'-').await?;
                self.stream.write_all(val.as_bytes()).await?;
                self.stream.write_all(b"\r\n").await?;
            }
            Frame::Integer(val) => {
                self.stream.write_u8(b':').await?;
                self.write_decimal(*val).await?;
            }
            Frame::Null => {
                self.stream.write_all(b"$-1\r\n").await?;
            }
            Frame::Bulk(val) => {
                let len = val.len();

                self.stream.write_u8(b'$').await?;
                self.write_decimal(len as u64).await?;
                self.stream.write_all(val).await?;
                self.stream.write_all(b"\r\n").await?;
            }
            Frame::Array(_val) => unimplemented!(),
        }

        self.stream.flush().await?;

        Ok(())
    }

    pub async fn write_decimal(&mut self, val: u64) -> io::Result<()> {
        let s = val.to_string();
        self.stream.write_all(s.as_bytes()).await?;
        self.stream.write_all(b"\r\n").await
    }

    fn parse_frame(&mut self) -> Result<Option<Frame>> {
        // 创建 `T: Buf` 类型
        let mut buf = Cursor::new(&self.buffer[..]);

        // 检查是否读取了足够解析出一个帧的数据
        match Frame::check(&mut buf) {
            Ok(_) => {
                // 获取组成该帧的字节数
                let len = buf.position() as usize;

                // 在解析开始之前，重置内部的游标位置
                buf.set_position(0);

                // 解析帧
                let frame = Frame::parse(&mut buf)?;

                // 解析完成，将缓冲区该帧的数据移除
                self.buffer.advance(len);

                // 返回解析出的帧
                Ok(Some(frame))
            }
            // 缓冲区的数据不足以解析出一个完整的帧
            Err(Incomplete) => Ok(None),
            // 遇到一个错误
            Err(e) => Err(e.into()),
        }
    }
}

// async fn process_v1(socket: TcpStream) {
//     // `Connection` 对于 redis 的读写进行了抽象封装
//     // 因此我们读到的是一个一个数据帧frame(数据帧 = redis命令 + 数据)，而不是字节流
//     // `Connection` 是在 mini-redis 中定义
//     let mut connection = Connection::new(socket);

//     if let Some(frame) = connection.read_frame().await.unwrap() {
//         println!("Got frame: {:?}", frame);
//         // 回复一个错误
//         let response = Frame::Error("unimplemented".to_string());
//         connection.write_frame(&response).await.unwrap();
//     }
// }

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
                    println!("server GET {} = {:?}", key, value);
                    Frame::Bulk(value.clone().into())
                } else {
                    println!("server GET {} = None", key);
                    Frame::Null
                }
            }

            cmd => panic!("unimplemented {:?}", cmd),
        };

        connection.write_frame(&response).await.unwrap();
    }
}
