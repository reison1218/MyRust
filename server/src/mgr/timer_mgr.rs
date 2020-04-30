use super::*;
use crate::mgr::game_mgr::GameMgr;
use chrono::format::Numeric::Month;
use chrono::{Datelike, Local, Timelike, Utc};
use std::sync::RwLock;
use std::thread;
use std::time::Duration;
use std::time::SystemTime;

///初始化定时器任务函数
pub fn init(gm: Arc<RwLock<GameMgr>>) {
    let time = SystemTime::now();
    //每日零点任务
    zero_day(gm.clone());
    //每5分钟保存玩家数据
    save_timer(gm.clone());
    info!(
        "定时任务初始化完毕!耗时:{:?}ms",
        time.elapsed().unwrap().as_millis()
    )
}

///每日零点执行的任务
fn zero_day(gm: Arc<RwLock<GameMgr>>) {
    let mut next_time_tmp: i64 = 0;
    //每天0点执行
    let zero_day = move || loop {
        let now_time = Local::now();
        let next_time = now_time
            .with_hour(23)
            .unwrap()
            .with_minute(59)
            .unwrap()
            .with_second(59)
            .unwrap();
        let res = next_time.timestamp() - now_time.timestamp();
        if next_time_tmp == next_time.timestamp() {
            std::thread::sleep(Duration::from_secs(2 as u64));
            continue;
        }
        std::thread::sleep(Duration::from_secs(res as u64));
        info!("开始执行0点任务");
        let now_time = SystemTime::now();
        let mut write = gm.write().unwrap();
        for u in write.users.values_mut() {
            u.day_reset();
            u.update();
        }
        info!(
            "零点重制完成！重制玩家数量:{},耗时{:?}ms",
            write.users.len(),
            now_time.elapsed().unwrap().as_millis()
        );
        next_time_tmp = next_time.timestamp();
    };
    std::thread::spawn(zero_day);
}

///保存玩家数据的定时器任务函数
fn save_timer(gm: Arc<RwLock<GameMgr>>) {
    let (sender, rec) = std::sync::mpsc::channel();

    let m = move || loop {
        let gm = gm.clone();
        gm.write().unwrap().save_user(sender.clone());
        let d = Duration::from_secs(60 * 5);
        std::thread::sleep(d);
    };

    let re = move || loop {
        let res = rec.recv();
        let mut count = 0;
        match res {
            Err(str) => {
                error!("玩家数据保存出错,message:{:?}", str);
            }
            Ok(mut vec) => {
                let time = std::time::SystemTime::now();
                for mut v in vec {
                    let rs = v.update();
                    match rs {
                        Ok(r) => {
                            count += 1;
                        }
                        Err(e) => {}
                    }
                }
                info!(
                    "执行定时保存玩家结束，执行数量:{},耗时:{}ms",
                    count,
                    time.elapsed().unwrap().as_millis()
                );
            }
        }
    };
    std::thread::spawn(m);
    std::thread::spawn(re);
}
