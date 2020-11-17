mod web;
mod tcp_client;
mod web_socket;
mod mio_test;
mod map;
mod test_async;
mod behavior_test;
mod test_tokio;
mod sig_rec;
use serde_json::json;
use std::time::{Duration, SystemTime, Instant};
use protobuf::Message;
use num_enum::TryFromPrimitive;
use num_enum::IntoPrimitive;
use num_enum::FromPrimitive;
use log::{info, LevelFilter};
use serde::{Serialize, Deserialize, Serializer};


//use tcp::thread_pool::{MyThreadPool, ThreadPoolHandler};
// use tcp::tcp::ClientHandler;
// use tcp::util::bytebuf::ByteBuf;
// use tcp::util::packet::Packet;

use std::collections::{HashMap, BinaryHeap, LinkedList, HashSet};
use std::sync::mpsc::{Receiver, channel};

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


use std::io::{Write, Read};
use tools::tcp::{ClientHandler, new_tcp_client};
use tools::util::packet::Packet;
use std::collections::btree_map::Entry::Vacant;
use std::collections::binary_heap::PeekMut;
use crate::web::{test_http_server, test_faster};
use crate::web::test_http_client;
use threadpool::ThreadPool;
use std::any::Any;
use envmnt::{ExpandOptions, ExpansionType};
use std::ops::{DerefMut, Deref};
use rand::prelude::*;
use std::collections::BTreeMap;
use std::alloc::System;
use std::cell::{Cell, RefCell, RefMut};
use serde_json::Value;
use serde::private::de::IdentifierDeserializer;
use std::str::FromStr;
use std::sync::{Arc, RwLock, Mutex, Condvar};
use std::sync::atomic::AtomicU32;
use tools::redis_pool::RedisPoolTool;
use tools::util::bytebuf::ByteBuf;
use std::panic::catch_unwind;
use std::fs::File;
use std::env;
use chrono::{Local, Datelike, Timelike};
use std::fmt::{Display, Debug};
use std::mem::Discriminant;
use futures::executor::block_on;
use std::thread::{Thread, JoinHandle};
use rayon::prelude::ParallelSliceMut;
use futures::SinkExt;
use std::borrow::{Borrow, BorrowMut};
use std::hash::Hasher;
use std::rc::Rc;
use futures::join;
use crate::test_async::async_main;
use std::collections::btree_map::Range;
use tools::templates::template::{init_temps_mgr, TemplatesMgr};
use crate::map::generate_map;
use actix::{Actor, SyncArbiter, ContextFutureSpawner};
use std::convert::TryInto;
use crossbeam::atomic::{AtomicConsume, AtomicCell};
use tools::macros::GetMutRef;
use futures::future::join3;
use crossbeam::sync::ShardedLock;
use tools::protos::room::C_LEAVE_ROOM;

#[macro_use]
extern crate lazy_static;
extern crate proc_macro;

lazy_static! {
    static ref ID:Arc<RwLock<AtomicU32>>={
        let id:Arc<RwLock<AtomicU32>> = Arc::new(RwLock::new(AtomicU32::new(1011000025)));
        id
    };

    ///静态配置文件
    static ref TEMPLATES: TemplatesMgr = {
        let path = env::current_dir().unwrap();
        let str = path.as_os_str().to_str().unwrap();
        let res = str.to_string()+"/template";
        let conf = init_temps_mgr(res.as_str());
        conf
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

fn test_tcp_client(){
    for i in 0..=1{
        let m = move ||{
            let mut str = "test".to_owned();
            str.push_str(i.to_string().as_str());
            tcp_client::test_tcp_client(str.as_str());
        };
        std::thread::spawn(m);
        std::thread::sleep(Duration::from_millis(2000));
    }
     //std::thread::sleep(Duration::from_millis(40000));
    tcp_client::test_tcp_client("test");
}

fn test_binary(){
    // let int = 123u32;
    // //(1)最原始直接基础的位操作方法。
    // let mut byte: u8 = 0b0000_0000;
    // println!("{:0x}", int);
    // byte |= 0b0000_1000; // Set a bit
    // println!("0b{:08b}", byte);
    // byte &= 0b1111_0111; // Unset a bit
    // println!("0b{:08b}", byte);
    // byte ^= 0b0000_1000; // Toggle a bit
    // println!("0b{:08b}", byte);
    // byte = !byte; // Flip all bits
    // println!("0b{:08b}", byte);
    // byte <<= 1; // shift left one bit
    // println!("0b{:08b}", byte);
    // byte >>= 1; // shift right one bit
    // println!("0b{:08b}", byte);
    // //特别提醒：rust为每一个数字类型都实现了大量方法，其中包括位操作方法！！！具体请参看下方链接！！！
    // //https://doc.rust-lang.org/std/primitive.u8.html
    // let mut rbyte: u8 = 0b1000_0000;
    // rbyte = rbyte.rotate_left(1); // rotate left one bit
    // println!("0b{:08b}", byte);
    // //https://doc.rust-lang.org/std/#primitives
    // rbyte = rbyte.rotate_right(1); // rotate right one bit
    // println!("0b{:08b}", rbyte);
    // bit_twiddling(0, 3);
    // bit_twiddling(8, 3);
    //test bitwise operation macros
    // assert_eq!(eq1!(0b0000_1111, 0), true);
    // assert_eq!(eq0!(0b0000_1111, 4), true);
    // assert_eq!(set!(0b0000_1111, 0), 0x0f);
    // assert_eq!(clr!(0b0000_1111, 0), 0x0e);
}



// macro_rules! test{
//
//     ($key:expr=>$value:expr,$yunsuan:ident)=>{
//         if $key  $yunsuan $value{
//             true
//         }else{
//         false
//         }
//     };
// }

// {
// "panding": {
// "cell_type": 1,
// "yunsuanfu": ">",
// "canshu": 1
// },
// "result":{"true":[1001,1002],"false":[1004]}
// }
#[derive(Debug, Clone, Eq, PartialEq, TryFromPrimitive,IntoPrimitive)]
#[repr(u8)]
enum  HH{
    AA=1,
}

struct  TT{
    s:String,
}

impl PartialEq for TT{
    fn eq(&self, other: &Self) -> bool {
        self.s.eq_ignore_ascii_case(other.s.as_str())
    }
}

impl std::cmp::Eq for TT{

}

impl std::hash::Hash for TT{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.s.hash(state)
    }
}



// impl std::cmp::PartialEq<HH> for TT{
//     fn eq(&self, other: &HH) -> bool {
//         self.s == *other
//     }
// }

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

#[derive(Debug,Default)]
struct Foo {
    x: i32,
    y:String,
}

impl  Foo{
    pub fn get_x(&self)->i32{
        self.x
    }
}

impl Deref for Foo{

    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.y
    }
}


#[derive(Default)]
struct BaseFoo{
    foo:Option<Foo>
}

fn test_err()->anyhow::Result<()>{
    anyhow::bail!("test");
}

#[derive(Debug)]
pub struct Form<T:Sized>{p: T}

impl<T> Form<T> {
    /// Deconstruct to an inner value
    pub fn into_inner(self) -> T {
        self.p
    }
}

trait Layoutable {
    fn position(&self) -> (f32,f32);
    fn size(&self) -> (f32,f32);
    fn set_position(&mut self, x: f32, y: f32);
    fn set_size(&mut self, width: f32, height: f32);
}
macro_rules! impl_layoutable {
    ($e: ty) => {
        impl Layoutable for $e {
            fn position(&self) -> (f32,f32) { self.pos }
            fn size(&self) -> (f32,f32) { self.size }
            fn set_position(&mut self, x: f32, y: f32) { self.pos = (x, y); }
            fn set_size(&mut self, width: f32, height: f32) { self.size = (width, height); }
        }
    };
}

#[derive(Default)]
struct TestMacro{
pos: (f32, f32),
size: (f32, f32)
}

impl_layoutable!(TestMacro);



trait DoSomething<T>{
    fn do_sth(&self,value:T);
}
impl <'a,T:Debug> DoSomething<T> for &'a usize{
    fn do_sth(&self, value: T) {
        println!("{:?}",value);
    }
}

// fn do_foo<'a>(b:Box<dyn DoSomething<&'a usize>>){
//     let s:usize = 10;
//     b.do_sth(&s);
// }

fn do_bar(b:Box<dyn for<'f> DoSomething<&'f usize>>){
    let s:usize = 10;
    b.do_sth(&s);
}

pub fn test_str<'b,'a:'b>(str:&'a str,str1: &'a str)->&'b str{
    if str.len()>str1.len() {
        return str
    }
    str1
}

thread_local! {
    pub static I:Cell<u32> = Cell::new(1);
}

#[derive(Debug)]
pub struct TestT {
    temp: &'static String,
}

impl Drop for TestT {
    fn drop(&mut self) {
        println!("drop Test");
    }
}

pub fn test_unsafe2(){
    let mut t: Option<TestT> = None;
    unsafe {
        let str = String::from("哈哈");
        let str_ptr = str.borrow() as *const String;
        t = Some(TestT {
            temp: str_ptr.as_ref().unwrap(),
        });
    }
    println!("res {:?}", t.as_ref().unwrap().temp);
}

#[derive(Default)]
pub struct TestSize{
    a:u32,
    b:u32,
    c:u32,
}

#[cfg(feature="bar")]
mod bar {
    pub fn bar() {
        println!("test");
    }
}

#[cfg(any(bar))]
mod ss{
    pub fn test(){
        println!("test");
    }
}


#[derive(Default,Debug,Clone)]
struct  STest{
    str:String,
    v:Vec<String>,
}

#[derive(Default,Clone)]
struct TestS{
    i:u32,
}

tools::get_mut_ref!(TestS);


fn main() -> anyhow::Result<()> {

    tcp_client::test_tcp_client("reison");
    // let mut arc=  Arc::new(RwLock::new(TestS::default()));
    // for i in 0..9999{
    //     let res = arc.clone();
    //     let m = move||{
    //         let read = res.read().unwrap();
    //     };
    //
    // }
    // test_unsafe();
    // let mut t = Test::default();
    // t.str.push_str("asdf");
    // t.i.fetch_add(1);
    // unsafe{
    //     let res:Test = std::mem::transmute_copy(&t);
    //     dbg!(res);
    //     dbg!(t);
    // }

    // rc1.borrow().borrow_mut().str.push_str("1");
    // rc2.borrow().borrow_mut().str.push_str("1");
    // tcp_client::test_tcp_client("reison1");
    // crate::bar::bar();
    // crate::ss::test();

    // let rc = RefCell::new(Test::default());
    // rc.borrow_mut().str.push_str("1");

    // let mut s1 = STest::default();
    // s1.str.push_str("s1");
    // s1.v.push("s1".to_owned());
    // let mut s2 = STest::default();
    // s2.str.push_str("s2");
    // s2.v.push("s2".to_owned());
    //
    //
    // std::mem::swap(&mut s1,&mut s2);
    // let (sender,rec) = crossbeam::channel::unbounded();
    // let res = async move {
    //     let time = std::time::SystemTime::now();
    //     for _ in 0..999999999{
    //         sender.send(1);
    //     }
    //     println!("send task time:{:?}",time.elapsed().unwrap());
    // };
    // async_std::task::spawn(res);
    //
    // let time = std::time::SystemTime::now();
    // let mut res:i32 = 0;
    // loop{
    //     res += rec.recv().unwrap();
    //     if res>=999999999{
    //         break;
    //     }
    // }
    // println!("rec task time:{:?},{}",time.elapsed().unwrap(),res);
    // tcp_client::test_tcp_client("reison");
    // let test = async_std::sync::Arc::new(async_std::sync::Mutex::new(Test::default()));
    // let res = async move{
    //     let t = test.clone();
    //     let s = async move{
    //         let lock = t.lock().await;
    //         dbg!("s:{:?}",std::thread::current().name().unwrap());
    //         ()
    //     };
    //
    //     let t1 = test.clone();
    //     let s1 = async move{
    //         let lock = t1.lock().await;
    //         dbg!("s1:{:?}",std::thread::current().name().unwrap());
    //         ()
    //     };
    //     let t2 = test.clone();
    //     let s2 = async move{
    //         let lock = t2.lock().await;
    //         dbg!("s2:{:?}",std::thread::current().name().unwrap());
    //         ()
    //     };
    //     let res = join3(s,s1,s2);
    //     task::spawn(res).await;
    //     ()
    // };
    //
    // async fn test_mutex(name:&str,test:Arc<Mutex<Test>>){
    //     let lock = test.lock().unwrap();
    //     dbg!("{:?}:{:?}",name,std::thread::current().name().unwrap());
    // }
    //
    // block_on(res);
    //
    // std::thread::sleep(Duration::from_millis(50000));
    //
    // test_channel_and_mutex();
    // test_channel();
    // let x = Box::new(&2usize);
    // do_bar(x);

    // And set a new one
    //hostname::set("potato")?;


    // let s: String = "Hello, World".to_string();
    // let any: Box<dyn Any> = Box::new(s);
    // let res:Box<String> = any.downcast().unwrap();
    // dbg!(res);
    // let t = TTT::default();
    // let res = t.d.take();
    // println!("{:?}", t.borrow().d.take());
    //test_faster();
    //tcp_client::test_tcp_client("reison");
    // let m = move||{
    //     loop{
    //
    //     }
    // };
    // std::thread::spawn(m);
    // let builder = std::thread::Builder::new();
    // // handler.join().unwrap();
    //
    // let handler = builder
    //     .spawn(|| {
    //         std::thread::current()
    //     })
    //     .unwrap();
    // handler.join().expect("Couldn't join on the associated thread");


    // let sleep_time = res.timestamp() - date.timestamp();
    // println!("{}",sleep_time);
    //let test = TestMacro::default();
    // let mut map= HashMap::new();
    // map.insert(1,Rc::new(RefCell::new(Form{p:String::new()})));
    // let res = map.get_mut(&1).unwrap();
    // let mut re = Cell::new(Form{p:String::new()});

    // println!("{:?}",v);
    // println!("{:?}",res);
    //tcp_client::test_tcp_clients();
    // let season_temp = TEMPLATES.get_season_temp_mgr_ref().get_temp(&1001).unwrap();
     //map::generate_map();
    // let a:u8 = HH::AA.into();
    // println!("{}",a)
    // let words:[u32;5] = [1,2,3,4,5];
    //
    // let id = 2_u32;
    // match id {
    //     ss@ =>{
    //
    //     }
    // }
    // let a = -1;
    // let b = 2;
    // let res = b+*&a;
    // println!("{}",res);
    // let mut k =&&&&&Foo{x:10,y:String::from("test")};
    // print!("{}",k.get_x());
    // println!("{:?}",k.bytes());

    //generate_map();
    // let foo = Foo{x:1};
    // let mut rc = Rc::new(foo);
    //
    //
    // block_on(async_main());



    //test_unsafe();
    //
    // let mut foo = Foo { x: 42 };
    //
    // let x = &mut foo.x;
    // *x = 13;
    // let y = foo;
    // println!("{:?}", (&y).x);  //only added this line
    // println!("{:?}", y.x); //13

    //let test = test!(1=>2,<);
    //crate::map::generate_map();
    // let i:u8 = 0;
    // let j = true;
    // println!("{}",std::mem::size_of_val(&i));
    // println!("{}",std::mem::size_of_val(&j));
    //test_binary();
    //test_sort();
    //test_tcp_client();
    //map::generate_map();
    // let res = Local::now().timestamp_millis();
    // println!("{}",res);
    //test_channel();
    //test_loop();
    Ok(())
}

fn test_loop(){
    let mut index = 1_i32;
    'out:loop{
        println!("start");
        loop{
            std::thread::sleep(Duration::from_millis(1000));
            println!("{}",index);
            index+=1;
            if index == 3{
                index = 1_i32;
                continue 'out;
            }
        }
    }
}

fn test_drop(){
    {
        let _a = Count(3);
        let _ = Count(2);
        let _c = Count(1);
    }
    {
        let _a = Count(3);
        let _b = Count(2);
        let _c = Count(1);
    }
}

struct Count(i32);

impl Drop for Count {
    fn drop(&mut self) {
        println!("dropping count {}", self.0);
    }
}

fn test_channel_and_mutex(){
    let test = Test::default();
    let arc = Arc::new(tokio::sync::RwLock::new(test));
    let metux_time = std::time::SystemTime::now();
    let mut size = 0;
    loop{
        size+=1;
        if size == 99999{
            break;
        }
        let arc_clone = arc.clone();
        let m =    async move  {
            let mut lock = arc_clone.write().await;
            lock.i.fetch_add(1);
            ()
        };
        // std::thread::spawn(m);
        let res = async_std::task::spawn(m);
        block_on(res);
    }
    // let mut builder = std::thread::Builder::new();
    // let res = builder.spawn(||{std::thread::current()}).unwrap();
    // res.join();
    println!("mutex time:{:?},value:{}",metux_time.elapsed().unwrap(),block_on(arc.write()).i.load());
    // println!("mutex time:{:?},value:{}",metux_time.elapsed().unwrap(),arc.write().await.i.load());

    // let res = async move{
    //     // async_std::task::sleep(Duration::from_millis(1000)).await;
    //     let lock = arc.write().await;
    //     println!("mutex time:{:?},value:{}",metux_time.elapsed().unwrap(),lock.i.load());
    // };
    // async_std::task::spawn(res);


    let (cb_sender,cb_rec) = crossbeam::channel::bounded(102400);
    let m = move||{
        let mut size = 0;
        let rec_time = std::time::SystemTime::now();
        loop{
            let res = cb_rec.recv();
            if let Err(e) = res{
                println!("{:?}",e);
                break;
            }
            size+=1;
            if size == 99999{
                println!("cb_rec time:{:?}",rec_time.elapsed().unwrap());
            }
        }
    };
    std::thread::spawn(m);
    let send_time = std::time::SystemTime::now();
    for i in 0..99999{
        cb_sender.send(Test::default());
    }
    println!("cb_send time:{:?}",send_time.elapsed().unwrap());

    std::thread::sleep(Duration::from_millis(50000));

}


fn test_channel(){
    let (std_sender,std_rec) = std::sync::mpsc::sync_channel(102400);
    let m = move||{
        let mut size = 0;
        let rec_time = std::time::SystemTime::now();
      loop{
          let res = std_rec.recv().unwrap();
          size+=1;
          if size == 9999999{
              println!("std_rec time:{:?}",rec_time.elapsed().unwrap());
          }
      }
    };
    std::thread::spawn(m);
    let send_time = std::time::SystemTime::now();
    for i in 0..9999999{
        std_sender.send(Test::default());
    }
    println!("std_send time:{:?}",send_time.elapsed().unwrap());

    let (cb_sender,cb_rec) = crossbeam::channel::bounded(102400);

    let m = move||{
        let mut size = 0;
        let rec_time = std::time::SystemTime::now();
        loop{
            let res = cb_rec.recv().unwrap();
            size+=1;
            if size == 9999999{
                println!("cb_rec time:{:?}",rec_time.elapsed().unwrap());
            }
        }
    };
    std::thread::spawn(m);
    let send_time = std::time::SystemTime::now();
    for i in 0..9999999{
        cb_sender.send(Test::default());
    }
    println!("cb_send time:{:?}",send_time.elapsed().unwrap());

    std::thread::sleep(Duration::from_millis(5000));

}



#[derive(Debug,Default)]
struct Test{
    pub str:String,
    pub i:AtomicCell<u32>,
}

unsafe impl Send for Test{}

unsafe impl Sync for Test{}

fn test_unsafe(){
    unsafe {

        let mut t = Test::default();
        let mut t2:Test = std::mem::transmute_copy(&t);
        t2.i.store(100);
        dbg!(t);
        dbg!(t2);
        // let mut test = Test{str:"test".to_owned(),i:AtomicCell::new(0)};
        // let test_p = &mut test as *mut Test;
        // let s = test_p.as_mut().unwrap();
        // let s1 = test_p.as_mut().unwrap();
        // s1.str.push_str("2");
        // s.str.push_str("1");
        // println!("{:?}",s);
        // println!("{:?}",s1);
        // let mut str = "test".to_owned();
        // let s_p = &str as *const String;
        // let s_p_m = &mut str as *mut String;
        // assert_eq!(s_p, s_p_m);
        // println!("s_p:{}", *s_p);
        // println!("s_p_m:{}", *s_p_m);
        // std::mem::drop(str);
        // let s_p_m = &mut *s_p_m;
        // s_p_m.push_str("sss");
        // println!("str:{:?}", s_p_m);
        //
        // let address = 0x7ffee3b103af_usize;
        // let s = address as *mut String;
        // println!("{:?}",s);
        // let s = &mut *s;
        // s.push_str("ss");
        // println!("{:?}",s);
    }
}
fn test_sort(){
    let mut v = Vec::new();
    let mut rng = thread_rng();
    for i in 1..=99999{
        let n: u32 = rng.gen_range(1, 99999);
        v.push(n);
    }

    let time = SystemTime::now();
    for i in 1..=9999{
        v.par_sort_by(|a,b|b.cmp(a));
    }
    //println!("{:?}",v);
    println!("rayon:{:?}",time.elapsed().unwrap());

    let mut v = Vec::new();
    let mut rng = thread_rng();
    for i in 1..=99999{
        let n: u32 = rng.gen_range(1, 99999);
        v.push(n);
    }
    let time = SystemTime::now();
    for i in 1..=9999{
        v.sort_by(|a,b|b.cmp(a));
    }
    //println!("{:?}",v);
    println!("comment:{:?}",time.elapsed().unwrap());
}


fn test()->impl Display{
    let res = "test".to_string();
    res
}