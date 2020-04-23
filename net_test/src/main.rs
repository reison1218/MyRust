mod web;
mod tcp_client;
mod web_socket;
mod mio_test;

use std::time::Duration;
use protobuf::Message;
//use tcp::thread_pool::{MyThreadPool, ThreadPoolHandler};
// use tcp::tcp::ClientHandler;
// use tcp::util::bytebuf::ByteBuf;
// use tcp::util::packet::Packet;
use futures::executor::block_on;
use std::collections::{HashMap, BinaryHeap};
use std::sync::mpsc::Receiver;

//use tokio::net::{TcpListener as TokioTcpListener,TcpStream as TokioTcpStream};
//use tokio::prelude::*;
//use tokio::runtime::Runtime as TokioRuntime;
//use tokio::net::tcp::{ReadHalf,WriteHalf};
use std::error::Error;
//use std::io::{Read, Write};
use std::net::{TcpStream, TcpListener};

use async_std::io;
use async_std::net::{TcpListener as AsyncTcpListener, TcpStream as AsyncTcpStream};
use async_std::prelude::*;
use async_std::task;

use tools::protos::base::MessPacketPt;

use std::sync::{Arc, RwLock};
use std::io::{Write, Read};
use tools::tcp::ClientHandler;
use tools::util::packet::Packet;
use std::collections::btree_map::Entry::Vacant;
use std::collections::binary_heap::PeekMut;
use crate::web::test_http_server;
use crate::web::test_http_client;
use threadpool::ThreadPool;
use std::any::Any;
use envmnt::{ExpandOptions, ExpansionType};
use std::ops::DerefMut;
use rand::prelude::*;


macro_rules! test{
    ($a:expr)=>{
        if $a>0 {
            println!("{}",$a);
        };
    };
}

macro_rules! map{
    (@unit $($x:tt)*) => (());
    (@count $($rest:expr),*)=>(<[()]>::len(&[$(map!(@unit $rest)),*]));
    ($($key:expr=>$value:expr$(,)*)*)=>{
    {
        let cap = map!(@count $($key),*);
        let mut _map = std::collections::HashMap::with_capacity(cap);
        $(
         _map.insert($key,$value);
        )*
        _map
    };
    };
}

fn foo(words: &[&str]) {
    match words {
        // Ignore everything but the last element, which must be "!".
        [.., "!"] => println!("!!!"),

        // `start` is a slice of everything except the last element, which must be "z".
        [start @ .., "z"] => println!("starts with: {:?}", start),

        // `end` is a slice of everything but the first element, which must be "a".
        ["a", hh  @..] => println!("ends with: {:?}", hh),

        rest => println!("{:?}", rest),
    }
}

// async fn new_tokio_client(mut stream:TokioTcpStream){
//     let (mut read,mut write) = stream.split();
//     let read_s = async move{
//         println!("start write");
//         let mut bytes:[u8;1024] = [0;1024];
//         loop{
//             let size = read.read(&mut bytes[..]).await.unwrap();
//             println!("{:?}",&bytes[..]);
//         }
//     };
//     let write_s = async move{
//         println!("start write");
//         let mut bytes_w:[u8;1024] = [0;1024];
//         write.write(&mut bytes_w);
//         write.flush();
//     };
//     tokio::task::spawn(read_s);
//     tokio::task::spawn(write_s);
//     println!("new client!");
// }

// fn test_tokio(){
//     let mut runtime = TokioRuntime::new().unwrap();
//     let tcp_server = async{
//         let mut listener = TokioTcpListener::bind("127.0.0.1:8080").await.unwrap();
//         while let Some(stream) = listener.next().await {
//             match stream {
//                 Ok(mut stream) => {
//                     stream.set_recv_buffer_size(1024*32 as usize);
//                     stream.set_send_buffer_size(1024*32 as usize);
//                     stream.set_linger(Some(Duration::from_secs(5)));
//                     stream.set_keepalive(Some(Duration::from_secs(3600)));
//                     stream.set_nodelay(true);
//                     new_tokio_client(stream);
//                     println!("new client!");
//                 },
//                 Err(e) => { /* connection failed */ }
//             }
//         }
//     };
//     runtime.block_on(tcp_server);
// }

async fn test_async_std(){
    let mut listener = async_std::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();
    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next().await {
        println!("new client!");
        let mut read_stream = stream.unwrap();
        let mut write_stream = read_stream.clone();
        let read =  async move{
            println!("start read");
            let mut bytes:[u8;1024] = [0;1024];
            loop{
                let size = read_stream.read(&mut bytes).await.unwrap();
                println!("{}",size);
            }
        };

        let write = async move{
            println!("start write");
            let mut bytes:[u8;1024] = [0;1024];
            write_stream.write_all(&bytes[..]);
        };
        async_std::task::spawn(read);
        async_std::task::spawn(write);
    }
}

fn main() -> io::Result<()> {

    //tcp_client::test_tcp_client();
    // web_socket::test_websocket();
    //template::Templates::init("");
    // let mut name = "test.json".to_string();
    // let beta_offset = name.find('.').unwrap_or(name.len());
    //
    // name.replace_range(beta_offset.., "");
    // println!("{:?}",name);
    // std::thread::sleep(Duration::from_secs(10000));
    // mio_test::mio_test();
    // if !envmnt::exists("MY_ENV_VAR") {
    //     envmnt::set("MY_ENV_VAR", "SOME VALUE");
    // }
    let a:u32 = random();
    println!("{}",a);

    // We can also interact with iterators and slices:
    let mut rng = thread_rng();
    let arrows_iter = "➡⬈⬆⬉⬅⬋⬇⬊".chars();
    println!("Lets go in this direction: {}", arrows_iter.choose(&mut rng).unwrap());
    let mut nums = [1, 2, 3, 4, 5];
    nums.shuffle(&mut rng);
    println!("I shuffled my {:?}", nums);
    Ok(())
}

fn test_channel(){

    let (sender,rec) = std::sync::mpsc::sync_channel(1024000);

    let time = std::time::SystemTime::now();
    for i in 0..1000
    {
        let sender_cp = sender.clone();
        let m = move ||{
            for i in 0..1000{
                sender_cp.send(1);
            }
        };
        std::thread::spawn(m);
    }
    let mut i = 1;
    loop{
        let res = rec.recv();
         i += res.unwrap();
        if i >= 1000000{
            break;
        }
    }
    println!("channel:{}ms,{}",time.elapsed().unwrap().as_millis(),i);
    let time = std::time::SystemTime::now();
    let test=Arc::new(RwLock::new(Test{i:0}));
    for i in 0..1000{
        let t_cp = test.clone();
        let m = move ||{
            for j in 0..1000{
                let i = t_cp.write().unwrap().i;
                t_cp.write().unwrap().i+=1;
            }
        };
        let j = std::thread::spawn(m);
        j.join();
    }
    let test_cp = test.clone();
    // loop{
    //     let t = test_cp.read().unwrap();
    //     if t.i>=100000{
            println!("thread:{}ms,{}",time.elapsed().unwrap().as_millis(),test.read().unwrap().i);
    //         break;
    //     }
    // }
}

struct  Test{
    pub i:u32
}





