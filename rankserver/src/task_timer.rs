use crate::mgr::rank_mgr::RankMgr;
use async_std::sync::Mutex;
use async_std::task;
use log::info;
use rayon::slice::ParallelSliceMut;
use std::cmp::Ordering;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

///初始化定时器任务函数
pub fn init_timer(rm: Arc<Mutex<RankMgr>>) {
    let time = SystemTime::now();
    //每5分钟保存玩家数据
    update_rank(rm.clone());
    info!(
        "定时任务初始化完毕!耗时:{:?}ms",
        time.elapsed().unwrap().as_millis()
    )
}

fn update_rank(rm: Arc<Mutex<RankMgr>>) {
    let m = async move {
        loop {
            async_std::task::sleep(Duration::from_secs(60 * 5)).await;
            let mut lock = rm.lock().await;
            lock.rank_vec.par_sort_by(|a, b| {
                //如果段位等级一样
                if a.league.get_league_id() == b.league.get_league_id() {
                    //看时间
                    if a.league.league_time < b.league.league_time {
                        return Ordering::Greater;
                    }
                }
                //段位不一样直接看分数
                b.get_score().cmp(&a.get_score())
            })
        }
    };
    async_std::task::spawn(m);
}