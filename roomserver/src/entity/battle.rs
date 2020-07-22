use super::*;
use crate::entity::character::BattleCharacter;
use crate::entity::map_data::{Cell, CellType, TileMap};
use crate::handlers::battle_handler::Find;
use crate::task_timer::{Task, TaskCmd};
use crate::TEMPLATES;
use log::{error, info, warn};
use std::borrow::Borrow;
use std::collections::HashMap;
use std::str::FromStr;
use tools::tcp::TcpSender;

///技能判定枚举
pub enum SkillJudge {
    ChangeIndex = 1,                   //换地图块位置
    ShowIndex = 2,                     //展示地图块
    AddSkillDamgeBuff = 3,             //加技能伤害buff
    Hurted = 4,                        //受到伤害
    SkillDamgeNearCterAndCure = 5,     //对相临角色造成技能伤害并回血
    SkillDamgeAndChangeSkill = 6,      //造成技能伤害，使用后变成其他技能
    AutoPairCell = 7,                  //自动配对地图块
    LockUnPairCell = 8,                //锁定未配对的地图块
    MoveCter = 9,                      //移动一个角色
    SkillDamge10 = 10,                 //造成3点技能伤害
    SkillRangeDamgeAndCenterMore = 11, //范围技能伤害，中心更高
    SkillDamge12 = 12,                 //造成2点技能伤害
}

//技能消耗类型
pub enum SkillConsumeType {
    Energy = 1, //能量
}

///回合行为类型
#[derive(Clone, Debug, PartialEq)]
pub enum BattleCterState {
    Alive = 0,
    Die = 1,
}

///回合行为类型
#[derive(Clone, Debug, PartialEq)]
pub enum ActionType {
    None = 0,    //无效值
    Attack = 1,  //普通攻击
    UseItem = 2, //使用道具
    Skip = 3,    //跳过turn
    Open = 4,    //翻块
    Skill = 5,   //使用技能
}

impl From<u32> for ActionType {
    fn from(action_type: u32) -> Self {
        match action_type {
            1 => ActionType::Attack,
            2 => ActionType::UseItem,
            3 => ActionType::Skip,
            4 => ActionType::Open,
            5 => ActionType::Skill,
            _ => ActionType::None,
        }
    }
}

///行动单位
#[derive(Clone, Debug, Default)]
pub struct ActionUnit {
    pub team_id: u32,
    pub user_id: u32,
    pub turn_index: u32,
    pub actions: Vec<Action>,
}

#[derive(Clone, Debug, Default)]
pub struct Action {
    action_type: u8,
    action_value: u32,
}
///房间战斗数据封装
#[derive(Clone, Debug)]
pub struct BattleData {
    pub tile_map: TileMap,                          //地图数据
    pub choice_orders: Vec<u32>,                    //选择顺序里面放玩家id
    pub next_choice_index: usize,                   //下一个选择的下标
    pub next_turn_index: usize,                     //下个turn的下标
    pub turn_action: ActionUnit,                    //当前回合数据单元封装
    pub turn_orders: Vec<u32>,                      //turn行动队列，里面放玩家id
    pub battle_cter: HashMap<u32, BattleCharacter>, //角色战斗数据
    task_sender: crossbeam::Sender<Task>,           //任务sender
    sender: TcpSender,                              //sender
}

impl BattleData {
    pub fn new(task_sender: crossbeam::Sender<Task>, sender: TcpSender) -> Self {
        BattleData {
            tile_map: TileMap::default(),
            choice_orders: Vec::new(),
            next_choice_index: 0,
            next_turn_index: 0,
            turn_action: ActionUnit::default(),
            turn_orders: Vec::new(),
            battle_cter: HashMap::new(),
            task_sender,
            sender,
        }
    }

    //检测地图块是否可以翻
    pub fn check_choice_index(&self, index: usize) -> bool {
        let res = self.tile_map.map.get(index);
        if res.is_none() {
            return false;
        }
        let cell = res.unwrap();
        //校验地图块合法性
        if cell.id < CellType::Valid as u32 {
            return false;
        }
        //校验地图块是否被锁住
        if cell.check_is_locked() {
            return false;
        }
        //校验是否是世界块
        if cell.is_world {
            return false;
        }
        true
    }

    ///下个回合
    pub fn next_turn(&mut self) {
        //结算上一回合
        self.settlement_last_turn();
        //开始回合触发
        self.turn_start_trigger();

        //计算下一个回合
        self.next_turn_index += 1;
        if self.next_turn_index > self.turn_orders.len() - 1 {
            self.next_turn_index = 0;
        }
        //todo 通知客户端
    }

    pub fn get_next_turn_user(&self) -> u32 {
        let index = self.next_turn_index;
        *self.turn_orders.get(index).unwrap()
    }

    ///翻地图块
    pub fn open_cell(&mut self, index: usize) {
        //todo 此处应该计算技能cd
        let user_id = self.get_next_turn_user();

        unsafe {
            let battle_cters = &mut self.battle_cter as *mut HashMap<u32, BattleCharacter>;

            let battle_cter = battle_cters.as_mut().unwrap().get_mut(&user_id).unwrap();

            let recently_open_cell_index = battle_cter.recently_open_cell_index;
            let recently_open_cell_id = self
                .tile_map
                .map
                .get_mut(recently_open_cell_index)
                .unwrap()
                .id;

            let cell = self.tile_map.map.get_mut(index).unwrap() as *mut Cell;
            let cell = &mut *cell;

            let last_cell =
                self.tile_map.map.get_mut(recently_open_cell_index).unwrap() as *mut Cell;
            let last_cell = &mut *last_cell;
            let cell_id = cell.id;
            //如果配对了，则修改地图块配对的下标
            if cell_id == recently_open_cell_id {
                cell.pair_index = Some(recently_open_cell_index);
                last_cell.pair_index = Some(index);
            }

            //判断改地图块上面有没有角色，有的话将目标位置的玩家挪到操作玩家的位置上
            if cell.user_id > 0 {
                let target_cter = self.battle_cter.get_mut(&cell_id).unwrap();
                target_cter.cell_index = battle_cter.cell_index;

                let source_cell = self.tile_map.map.get_mut(battle_cter.cell_index).unwrap();
                source_cell.user_id = target_cter.user_id;
            }
            //改变角色位置
            battle_cter.cell_index = index;
            cell.user_id = battle_cter.user_id;

            //更新最近一次翻的下标
            battle_cter.recently_open_cell_index = index;

            //翻块次数-1
            battle_cter.residue_open_times -= 1;
        }

        //todo 检测地图块有没有陷阱

        //todo 下发到客户端

        //下一个turn
        self.next_turn();
    }

    ///回合开始触发
    pub fn turn_start_trigger(&mut self) {
        //todo
        //创建战斗检测定时器任务
        self.build_battle_turn_task();
    }

    ///结算上一回合
    pub fn settlement_last_turn(&mut self) {
        //todo
    }

    ///获得玩家回合下标
    pub fn get_turn_index(&self, user_id: u32) -> isize {
        let mut index = 0_isize;
        for member_id in self.turn_orders.iter() {
            if member_id == &user_id {
                return index;
            }
            index += 1;
        }
        return -1;
    }

    ///普通攻击
    pub fn attack(&mut self, user_id: u32, targets: Vec<u32>) {
        let damege = self.calc_damage(user_id);
        for target_id in targets.iter() {
            let reduce_damage = self.calc_reduce_damage(*target_id);
            let res = damege - reduce_damage;
            let battle_cter = self.battle_cter.get_mut(&user_id).unwrap();
            battle_cter.hp -= res as i32;
            if battle_cter.hp <= 0 {
                battle_cter.state = BattleCterState::Die as u8;
            }
            //todo 将计算结果推送给客户端
        }
    }

    ///计算伤害
    fn calc_damage(&self, user_id: u32) -> isize {
        let battle_cter = self.battle_cter.get(&user_id).unwrap();
        let damage = battle_cter.atk;
        //todo 此处应该加上角色身上的buff加成
        damage as isize
    }

    ///计算减伤
    fn calc_reduce_damage(&self, user_id: u32) -> isize {
        let battle_cter = self.battle_cter.get(&user_id).unwrap();
        let mut value = battle_cter.defence;
        //todo 此处应该加上角色身上的减伤buff
        value as isize
    }

    //跳过回合
    pub fn skip_turn(&mut self) {
        self.next_choice_index != 1;
        if self.next_turn_index > self.turn_orders.len() {
            self.next_turn_index = 0;
        }
        //返回客户端
    }

    ///校验是否翻过块
    pub fn check_is_open(&self) -> bool {
        if self.turn_action.actions.is_empty() {
            return false;
        }
        for action in self.turn_action.actions.iter() {
            let action_type = ActionType::from(action.action_type as u32);
            if action_type.eq(&ActionType::Open) {
                continue;
            }
            return true;
        }
        return false;
    }

    ///使用技能
    pub fn use_skill(&mut self, skill_id: u32, target_array: Option<Vec<u32>>) {
        let user_id = *self.turn_orders.get(self.next_turn_index).unwrap();
        //战斗角色
        let battle_cter = self.battle_cter.get_mut(&user_id).unwrap();
        //战斗角色身上的技能
        let skill = battle_cter.skills.get_mut((skill_id as usize)).unwrap();
        //技能判定
        let skill_judge = skill.skill_temp.skill_judge;

        //换地图块位置
        if skill_judge == SkillJudge::ChangeIndex as u16 {
            let target_array = target_array.unwrap();
            if target_array.len() < 2 {
                return;
            }
            let source_index = *target_array.get(0).unwrap() as usize;
            let target_index = *target_array.get(1).unwrap() as usize;
            self.change_index(user_id, source_index, target_index);
        }

        //展示地图块
        if skill_judge == SkillJudge::ShowIndex as u16 {
            if target_array.is_none() {
                return;
            }
            let target = target_array.unwrap();
            if target.is_empty() {
                return;
            }
            let index = target.get(0).unwrap() as usize;
            self.show_index(user_id, index);
        }

        //相临玩家造成3点伤害，持续1轮
        if skill_judge == SkillJudge::AddSkillDamgeBuff as u16 {
            self.damge_near_user_move_to(skill_id);
        }
    }

    //其他玩家移动到与你相邻的地图块时，对其造成3点伤害。持续1轮。
    pub fn damge_near_user_move_to(&mut self, skill_id: u32) {
        let user_id = self.get_next_turn_user();
        let battle_cter = self.battle_cter.get_mut(&user_id).unwrap();
        let skill = battle_cter.skills.get(skill_id as usize);
        //校验技能合法性
        if skill.is_none() {
            return;
        }
        let skill = skill.unwrap();
    }

    ///展示地图块
    pub fn show_index(&mut self, index: usize) {
        //校验index合法性
        let cell = self.tile_map.map.get(index);
        if cell.is_none() {
            return;
        }
        //校验index合法性
        let cell = cell.unwrap();
        if cell.id < CellType::Valid as u32 {
            return;
        }

        //校验是否世界块
        if cell.is_world {
            return;
        }

        //翻开的地图块不能展示
        if cell.pair_index > 0 {
            return;
        }

        //todo 下发给客户端
    }

    ///地图块换位置
    pub fn change_index(&mut self, user_id: u32, source_index: usize, target_index: usize) {
        let lock_skills = &TEMPLATES.get_skill_ref().lock_skills[..];
        let map_size = self.tile_map.map.len();
        //校验地图块
        if source_index > map_size || target_index > map_size {
            return;
        }
        let source_cell = self.tile_map.map.get(source_index).unwrap();
        let target_cell = self.tile_map.map.get(target_index).unwrap();

        //无效块不能换，锁定不能换
        if source_cell.id <= 1 || target_cell.id <= 1 {
            return;
        }
        //已配对的块不能换
        if source_cell.pair_index.is_some() || target_cell.pair_index.is_some() {
            return;
        }
        //锁定不能换
        for skill in source_cell.buff.iter() {
            if lock_skills.contains(&skill.id) {
                return;
            }
        }
        //锁定不能换
        for skill in target_cell.buff.iter() {
            if lock_skills.contains(&skill.id) {
                return;
            }
        }

        //先删掉
        let mut source_cell = self.tile_map.map.remove(source_index);
        let mut target_cell = self.tile_map.map.remove(target_index);

        //替换下标
        source_cell.index = target_index;
        target_cell.index = source_index;

        self.tile_map.map.insert(source_cell.index, source_cell);
        self.tile_map.map.insert(target_cell.index, target_cell);

        //todo 通知客户端
    }

    ///新建战斗回合定时器任务
    pub fn build_battle_turn_task(&self) {
        let next_turn_index = self.next_turn_index;
        let user_id = self.turn_orders.get(next_turn_index).unwrap();
        let time_limit = TEMPLATES
            .get_constant_ref()
            .temps
            .get("battle_turn_limit_time");
        let mut task = Task::default();
        if let Some(time) = time_limit {
            let time = u64::from_str(time.value.as_str());
            match time {
                Ok(time) => {
                    task.delay = time + 500;
                }
                Err(e) => {
                    task.delay = 5000_u64;
                    error!("{:?}", e);
                }
            }
        } else {
            task.delay = 5000_u64;
            warn!("the battle_turn_limit_time of Constant config is None!pls check!");
        }
        task.cmd = TaskCmd::ChoiceTurnOrder as u16;

        let mut map = serde_json::Map::new();
        map.insert("user_id".to_owned(), serde_json::Value::from(*user_id));
        task.data = serde_json::Value::from(map);
        let res = self.task_sender.send(task);
        if res.is_err() {
            error!("{:?}", res.err().unwrap());
        }
    }
}
