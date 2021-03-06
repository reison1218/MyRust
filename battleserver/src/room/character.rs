use crate::battle::battle_enum::buff_type::GD_ATTACK_DAMAGE;
use crate::battle::battle_enum::buff_type::{
    ADD_ATTACK, CHANGE_SKILL, NEAR_SUB_ATTACK_DAMAGE, SUB_ATTACK_DAMAGE,
};
use crate::battle::battle_enum::{AttackState, BattleCterState, TURN_DEFAULT_MOVEMENT_POINTS};
use crate::battle::battle_skill::Skill;
use crate::battle::mission::MissionData;
use crate::battle::{
    battle::{BattleData, Item},
    mission::MissionCompleteType,
};
use crate::battle::{battle_buff::Buff, mission::MissionResetType};
use crate::robot::robot_action::RobotStatusAction;
use crate::robot::robot_task_mgr::RobotTask;
use crate::robot::RobotData;
use crate::room::member::Member;
use crate::TEMPLATES;
use crossbeam::channel::Sender;
use log::{error, warn};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::str::FromStr;
use tools::macros::GetMutRef;
use tools::protos::base::{BattleCharacterPt, CharacterPt, LeaguePt, TargetPt};
use tools::templates::character_temp::{CharacterTemp, TransformInheritType};

#[derive(Clone, Debug, Default)]
pub struct Character {
    pub cter_id: u32,              //角色的配置id
    pub is_robot: bool,            //是否是机器人
    pub skills: Vec<u32>,          //玩家次角色所有已解锁的技能id,
    pub last_use_skills: Vec<u32>, //上次使用的技能
}

impl From<&CharacterPt> for Character {
    fn from(cter_pt: &CharacterPt) -> Self {
        let mut c = Character::default();
        c.cter_id = cter_pt.cter_id;
        c.skills = cter_pt.skills.clone();
        c.last_use_skills = cter_pt.last_use_skills.clone();
        c
    }
}

impl Into<CharacterPt> for Character {
    fn into(self) -> CharacterPt {
        let mut cter_pt = CharacterPt::new();
        cter_pt.set_cter_id(self.cter_id);
        cter_pt.set_skills(self.skills);
        cter_pt.set_last_use_skills(self.last_use_skills);
        cter_pt
    }
}

///角色战斗基础属性
#[derive(Clone, Debug, Default)]
pub struct BaseAttr {
    pub user_id: u32,   //玩家id
    pub cter_id: u32,   //角色的配置id
    pub name: String,   //名称
    pub grade: u8,      //等级
    pub atk: u8,        //攻击力
    pub hp: i16,        //角色血量
    pub defence: u8,    //角色防御
    pub energy: u8,     //角色能量
    pub max_energy: u8, //能量上限
    pub element: u8,    //角色元素
    pub item_max: u8,   //道具数量上限
}

///段位数据
#[derive(Clone, Debug)]
pub struct League {
    pub score: i32,       //段位积分
    pub league_time: i64, //进入段位时间
    pub league_id: i8,    //段位id
}

impl League {
    pub fn season_round_reset(&mut self) {
        let old_id = self.get_league_id();
        if old_id == 0 || old_id - 1 == 0 {
            self.league_id = 0;
            self.league_time = 0;
        } else {
            let res = crate::TEMPLATES
                .league_temp_mgr()
                .get_temp(&self.league_id)
                .unwrap();
            if old_id != self.league_id {
                self.score = res.score;
                let time = chrono::Local::now();
                self.league_time = time.timestamp_millis();
            }
        }
    }
    pub fn get_league_id(&self) -> i8 {
        self.league_id
    }

    pub fn into_pt(&self) -> LeaguePt {
        let mut lp = LeaguePt::new();
        lp.league_id = self.get_league_id() as i32;
        lp.league_score = self.score;
        lp.league_time = self.league_time;
        lp
    }

    pub fn update_score(&mut self, score: i32) -> i32 {
        self.score += score;
        if self.score < 0 {
            self.score = 0;
            return 0;
        }
        let mgr = crate::TEMPLATES.league_temp_mgr();
        if score < 0 {
            let league_temp = mgr.get_temp(&self.league_id);
            if let Err(_) = league_temp {
                self.league_id = 0;
            }
        }

        let league_temp = mgr.get_league_by_score(self.score);
        if league_temp.is_err() {
            return self.score;
        }
        let league_temp = league_temp.unwrap();
        //掉分了不能掉段位
        if self.score < league_temp.score {
            self.score = league_temp.score;
            return self.score;
        }

        if league_temp.id != self.get_league_id() {
            self.league_id = league_temp.id;
            let time = chrono::Local::now();
            self.league_time = time.timestamp_millis();
        }
        self.score
    }
}

impl From<&LeaguePt> for League {
    fn from(pt: &LeaguePt) -> Self {
        let mut l = League::default();
        l.league_time = pt.league_time;
        l.score = pt.league_score;
        l.league_id = pt.league_id as i8;
        l
    }
}

impl Default for League {
    fn default() -> Self {
        League {
            score: 0,
            league_time: 0,
            league_id: 0,
        }
    }
}

///角色战斗基础属性
#[derive(Clone, Debug, Default)]
pub struct BattleStatus {
    pub is_pair: bool,                //最近一次翻块是否匹配
    pub is_attacked: bool,            //一轮有没有受到攻击伤害
    is_can_end_turn: bool,            //是否可以结束turn
    pub locked_oper: u32,             //锁住的操作，如果有值，玩家什么都做不了
    pub state: BattleCterState,       //角色状态
    attack_state: AttackState,        //是否可以攻击
    pub pair_attack_open_count: bool, //配对攻击后奖励翻地图块次数,表示是否奖励过翻拍次数
}

///角色战斗buff
#[derive(Clone, Debug, Default)]
pub struct BattleBuff {
    pub buffs: HashMap<u32, Buff>,          //角色身上的buff
    pub passive_buffs: HashMap<u32, Buff>,  //被动技能id
    pub add_damage_buffs: HashMap<u32, u8>, //伤害加深buff key:buffid value:叠加次数
    pub sub_damage_buffs: HashMap<u32, u8>, //减伤buff  key:buffid value:叠加次数
}

impl BattleBuff {
    pub fn get_gd_buff(&mut self) -> Option<&mut Buff> {
        let mut buff_function_id;
        for buff in self.buffs.values_mut() {
            buff_function_id = buff.function_id;
            if buff_function_id == GD_ATTACK_DAMAGE[0] {
                return Some(buff);
            }
        }
        None
    }
}

///角色战斗流程相关数据
#[derive(Clone, Debug, Default)]
pub struct TurnFlowData {
    pub residue_movement_points: u8,   //剩余移动点数
    pub open_map_cell_vec: Vec<usize>, //最近一次turn翻过的地图块
    pub turn_limit_skills: Vec<u32>,   //turn限制技能
    pub round_limit_skills: Vec<u32>,  //round限制技能
    pub pair_limit_skills: Vec<u32>,   //配对限制技能
}

///角色战斗流程相关数据
#[derive(Clone, Debug, Default)]
pub struct IndexData {
    pub map_cell_index: Option<usize>,      //角色所在位置
    pub last_map_cell_index: Option<usize>, //上一次所在地图块位置
}

///商品数据
#[derive(Clone, Debug, Default)]
pub struct MerchandiseData {
    buy_map: HashMap<u32, u8>,      //购买商品次数
    buy_history: HashMap<u32, u16>, //购买记录
}

impl MerchandiseData {
    pub fn get_turn_buy_times(&self, merchandise_id: u32) -> u8 {
        let times = self.buy_map.get(&merchandise_id);
        let res = match times {
            Some(times) => *times,
            None => 0,
        };
        res
    }

    pub fn add_buy_times(&mut self, merchandise_id: u32) {
        let times = self.buy_map.get(&merchandise_id);
        let mut res = match times {
            Some(times) => *times,
            None => 0,
        };
        res += 1;
        self.buy_map.insert(merchandise_id, res);

        let times = self.buy_history.get(&merchandise_id);
        let mut res = match times {
            Some(times) => *times,
            None => 0,
        };
        res += 1;
        self.buy_history.insert(merchandise_id, res);
    }

    pub fn clear_turn_buy_times(&mut self) {
        self.buy_map.clear();
    }
}

///角色战斗数据
#[derive(Clone, Default)]
pub struct BattleCharacter {
    pub base_attr: BaseAttr,                               //基础属性
    pub league: League,                                    //单位数据
    pub status: BattleStatus,                              //战斗状态
    pub battle_buffs: BattleBuff,                          //战斗buff
    pub flow_data: TurnFlowData,                           //战斗流程相关数据
    pub index_data: IndexData,                             //角色位置数据
    pub merchandise_data: MerchandiseData,                 //商品数据
    pub gold: i32,                                         //金币
    pub revenge_user_id: u32,                              //复仇玩家
    pub mission_data: MissionData,                         //任务数据
    pub skills: HashMap<u32, Skill>,                       //玩家选择的主动技能id
    pub items: HashMap<u32, Item>,                         //角色身上的道具
    pub robot_data: Option<RobotData>, //机器人数据;如果有值，则是机器人，没有则是玩家
    pub self_transform_cter: Option<Box<BattleCharacter>>, //自己变身的角色
    pub self_cter: Option<Box<BattleCharacter>>, //原本的角色
}

tools::get_mut_ref!(BattleCharacter);

impl BattleCharacter {
    ///初始化战斗角色数据
    pub fn init(
        member: &Member,
        battle_data: &BattleData,
        robot_sender: Sender<RobotTask>,
    ) -> anyhow::Result<Self> {
        let is_robot = member.is_robot;
        let cter = &member.chose_cter;
        let mut battle_cter = BattleCharacter::default();
        let cter_id = cter.cter_id;
        battle_cter.base_attr.user_id = member.user_id;
        battle_cter.base_attr.cter_id = cter_id;
        battle_cter.base_attr.grade = member.grade;
        battle_cter.base_attr.name = member.nick_name.clone();
        let skill_ref = TEMPLATES.skill_temp_mgr();
        let buff_ref = TEMPLATES.buff_temp_mgr();
        for skill_id in cter.skills.iter() {
            let res = skill_ref.temps.get(skill_id);
            if res.is_none() {
                let str = format!("there is no skill for skill_id:{}!", skill_id);
                warn!("{:?}", str.as_str());
                anyhow::bail!(str)
            }
            let skill_temp = res.unwrap();
            let skill = Skill::from(skill_temp);
            battle_cter.skills.insert(*skill_id, skill);
        }
        let cter_temp = TEMPLATES.character_temp_mgr().get_temp_ref(&cter_id);
        if cter_temp.is_none() {
            let str = format!("cter_temp is none for cter_id:{}!", cter_id);
            warn!("{:?}", str.as_str());
            anyhow::bail!(str)
        }
        let cter_temp = cter_temp.unwrap();
        //初始化战斗属性,这里需要根据占位进行buff加成，但buff还没设计完，先放在这儿
        battle_cter.base_attr.hp = cter_temp.hp;
        battle_cter.base_attr.atk = cter_temp.attack;
        battle_cter.base_attr.defence = cter_temp.defence;
        battle_cter.base_attr.element = cter_temp.element;
        battle_cter.base_attr.energy = cter_temp.start_energy;
        battle_cter.base_attr.max_energy = cter_temp.max_energy;
        battle_cter.base_attr.item_max = cter_temp.usable_item_count;
        battle_cter.league = member.league.clone();
        cter_temp.passive_buff.iter().for_each(|buff_id| {
            let buff_temp = buff_ref.temps.get(buff_id).unwrap();
            let buff = Buff::from(buff_temp);
            battle_cter.add_buff(Some(battle_cter.get_user_id()), None, *buff_id, None);
            battle_cter
                .battle_buffs
                .passive_buffs
                .insert(*buff_id, buff);
        });

        battle_cter.reset_residue_movement_points();
        //处理机器人部分
        if is_robot {
            let robot_data = RobotData::new(
                battle_cter.get_user_id(),
                battle_data as *const BattleData,
                robot_sender,
            );
            battle_cter.robot_data = Some(robot_data);
        }
        Ok(battle_cter)
    }

    pub fn add_mission_progress(
        &mut self,
        value: u16,
        mission_type: MissionCompleteType,
        mission_parm: (u32, u32),
    ) -> bool {
        let res = self
            .mission_data
            .add_progress(value, mission_type, mission_parm);
        if res.0 {
            self.add_gold(res.1 as i32);
            return true;
        }
        false
    }

    pub fn add_gold(&mut self, value: i32) -> i32 {
        self.gold += value;
        if self.gold < 0 {
            self.gold = 0;
        }
        self.gold
    }

    pub fn sub_skill_cd(&mut self, value: Option<i8>) {
        self.skills.values_mut().for_each(|x| {
            let res;
            match value {
                Some(value) => res = value * -1,
                None => {
                    res = -1;
                }
            }
            x.add_cd(res);
        })
    }

    ///奖励移动点数
    pub fn pair_attack_reward_movement_points(&mut self) {
        self.change_attack_none();
        if !self.status.is_pair {
            return;
        }
        if self.status.pair_attack_open_count {
            return;
        }
        self.status.pair_attack_open_count = true;
        let temp = TEMPLATES.constant_temp_mgr();
        let res = temp.temps.get("turn_default_movement_points");
        let reward_count;
        match res {
            Some(res) => {
                let res = u8::from_str(res.value.as_str());
                match res {
                    Ok(res) => {
                        reward_count = res;
                    }
                    Err(e) => {
                        error!("{:?}", e);
                        reward_count = 2;
                    }
                }
            }
            None => {
                warn!("ConstantTemp could not find!the key is 'turn_default_movement_points'");
                reward_count = 2;
            }
        }

        self.flow_data.residue_movement_points += reward_count;
    }

    pub fn can_use_skill(&self) -> bool {
        for skill in self.skills.values() {
            if skill.cd_times > 0 {
                return false;
            }
        }
        true
    }

    pub fn get_robot_data_ref(&self) -> anyhow::Result<&RobotData> {
        if self.robot_data.is_none() {
            anyhow::bail!(
                "this battle_cter is not robot!user_id:{},cter_id:{}",
                self.base_attr.user_id,
                self.base_attr.cter_id
            )
        }
        Ok(self.robot_data.as_ref().unwrap())
    }

    pub fn robot_start_action(&self) {
        if self.robot_data.is_none() {
            return;
        }
        let res = self.get_mut_ref().robot_data.as_mut().unwrap();
        //开始仲裁
        res.thinking_do_something();
    }

    pub fn get_robot_action(&self) -> &mut Box<dyn RobotStatusAction> {
        let self_mut_ref = self.get_mut_ref();
        self_mut_ref
            .robot_data
            .as_mut()
            .unwrap()
            .robot_status
            .as_mut()
            .unwrap()
    }

    pub fn set_robot_action(&self, action: Box<dyn RobotStatusAction>) {
        let self_mut_ref = self.get_mut_ref();
        self_mut_ref.robot_data.as_mut().unwrap().robot_status = Some(action);
    }

    pub fn change_robot_status(&self, robot_action: Box<dyn RobotStatusAction>) {
        let res = self.get_robot_action();
        res.exit();
        self.set_robot_action(robot_action);
        let res = self.get_robot_action();
        res.enter();
    }

    pub fn get_cter_id(&self) -> u32 {
        self.base_attr.cter_id
    }

    pub fn get_user_id(&self) -> u32 {
        self.base_attr.user_id
    }

    ///是否行为被锁住了
    pub fn is_locked(&self) -> bool {
        self.status.locked_oper > 0
    }

    pub fn add_energy(&mut self, value: i8) {
        let v = self.base_attr.energy as i8;
        let max = self.base_attr.max_energy as i8;
        let res = v + value;
        if res < 0 {
            self.base_attr.energy = 0;
        } else {
            let result = res.min(max);
            self.base_attr.energy = result as u8;
        }
    }

    pub fn set_is_can_end_turn(&mut self, value: bool) {
        self.status.is_can_end_turn = value;
    }

    pub fn clear_turn_buy_times(&mut self) {
        self.merchandise_data.clear_turn_buy_times();
    }

    pub fn reset_mission(&mut self, reset_type: MissionResetType) {
        self.mission_data.reset(reset_type);
    }

    pub fn get_is_can_end_turn(&self) -> bool {
        self.status.is_can_end_turn
    }

    pub fn is_can_attack(&self) -> bool {
        self.status.attack_state == AttackState::Able
    }

    pub fn change_attack_able(&mut self) {
        self.status.attack_state = AttackState::Able;
    }

    pub fn change_attack_locked(&mut self) {
        self.status.attack_state = AttackState::Locked;
    }

    pub fn change_attack_none(&mut self) {
        self.status.attack_state = AttackState::None;
    }

    pub fn get_attack_state(&self) -> AttackState {
        self.status.attack_state
    }

    pub fn is_robot(&self) -> bool {
        self.robot_data.is_some()
    }

    ///从静态配置中初始化
    fn init_from_temp(&mut self, cter_temp: &CharacterTemp) {
        //先重制数据
        self.clean_all();
        //然后复制数据
        self.base_attr.cter_id = cter_temp.id;
        self.base_attr.element = cter_temp.element;
        self.base_attr.grade = 1;
        self.base_attr.hp = cter_temp.hp;
        self.base_attr.energy = cter_temp.start_energy;
        self.base_attr.max_energy = cter_temp.max_energy;
        self.base_attr.item_max = cter_temp.usable_item_count;
        self.base_attr.defence = cter_temp.defence;
        self.base_attr.atk = cter_temp.attack;
        self.status.state = BattleCterState::Alive;
        self.league = League::default();
        for skill_group in cter_temp.skills.iter() {
            for skill_id in skill_group.group.iter() {
                let skill_temp = TEMPLATES.skill_temp_mgr().get_temp(&skill_id);
                if let Err(e) = skill_temp {
                    warn!("{:?}", e);
                    continue;
                }
                let skill_temp = skill_temp.unwrap();
                let skill = Skill::from(skill_temp);
                self.skills.insert(skill.id, skill);
            }
        }
        cter_temp.passive_buff.iter().for_each(|buff_id| {
            let buff_temp = TEMPLATES.buff_temp_mgr().get_temp(buff_id);
            if let Ok(buff_temp) = buff_temp {
                let buff = Buff::from(buff_temp);
                self.add_buff(Some(self.get_user_id()), None, buff.get_id(), None);
                self.battle_buffs.passive_buffs.insert(*buff_id, buff);
            }
        });
    }

    ///变回来
    pub fn transform_back(&mut self) -> TargetPt {
        let clone;
        let is_self_transform;

        if self.base_attr.cter_id != self.self_transform_cter.as_ref().unwrap().base_attr.cter_id {
            is_self_transform = true;
            clone = self.self_transform_cter.as_mut().unwrap().clone();
        } else {
            clone = self.self_cter.as_mut().unwrap().clone();
            is_self_transform = false;
        }

        //拷贝需要继承的属性
        let transform_inherits = transform_inherit_copy(self, clone.base_attr.cter_id);

        //开始数据转换
        let _ = std::mem::replace(self, *clone);

        //处理保留数据
        self.transform_inherit(transform_inherits);

        //如果是从自己变身的角色变回去，则清空自己变身角色
        if is_self_transform {
            self.self_transform_cter = None;
        } else {
            self.self_cter = None;
        }

        let mut target_pt = TargetPt::new();
        let cter_pt = self.convert_to_battle_cter_pt();
        target_pt.set_transform_cter(cter_pt);
        target_pt
    }

    pub fn transform_inherit(&mut self, transform_inherits: Vec<TransformInherit>) {
        for ti in transform_inherits {
            let ti_type = ti.0;
            match ti_type {
                TransformInheritType::Other => {
                    let (open_map_cell_vec, is_can_end_turn) = ti.1.as_other().unwrap();
                    self.status.is_can_end_turn = is_can_end_turn;
                    self.flow_data.open_map_cell_vec = open_map_cell_vec;
                }
                TransformInheritType::Hp => {
                    self.base_attr.hp = ti.1.as_usize().unwrap() as i16;
                }
                TransformInheritType::Attack => {
                    self.base_attr.atk = ti.1.as_usize().unwrap() as u8;
                }
                TransformInheritType::ResidueOpenTimes => {
                    self.flow_data.residue_movement_points = ti.1.as_usize().unwrap() as u8;
                }
                TransformInheritType::AttackState => {
                    let res = ti.1.as_usize().unwrap() as u8;
                    let attack_state = AttackState::try_from(res).unwrap();
                    self.status.attack_state = attack_state;
                }
                TransformInheritType::MapIndex => {
                    self.index_data.map_cell_index = Some(ti.1.as_usize().unwrap());
                }
                TransformInheritType::Energy => {
                    self.base_attr.energy = ti.1.as_usize().unwrap() as u8;
                }
                TransformInheritType::Gold => {
                    self.gold = ti.1.as_usize().unwrap() as i32;
                }
                _ => {}
            }
        }
    }

    ///变身
    pub fn transform(
        &mut self,
        from_user: u32,
        cter_id: u32,
        buff_id: u32,
        next_turn_index: usize,
    ) -> anyhow::Result<TargetPt> {
        let cter_temp = TEMPLATES.character_temp_mgr().get_temp_ref(&cter_id);
        if cter_temp.is_none() {
            anyhow::bail!("cter_temp can not find!cter_id:{}", cter_id)
        }
        let cter_temp = cter_temp.unwrap();
        //拷贝需要继承的属性
        let transform_inherits = transform_inherit_copy(self, cter_id);

        //保存原本角色
        if self.self_cter.is_none() {
            self.self_cter = Some(Box::new(self.clone()));
        }
        //初始化数据成另外一个角色
        self.init_from_temp(cter_temp);

        //将继承属性给当前角色
        self.transform_inherit(transform_inherits);

        //给新变身加变身buff
        let buff_temp = TEMPLATES.buff_temp_mgr().get_temp(&buff_id);
        if let Err(e) = buff_temp {
            warn!("{:?}", e);
            anyhow::bail!("")
        }
        let buff_temp = buff_temp.unwrap();
        let buff = Buff::new(buff_temp, Some(next_turn_index), None, None);
        self.battle_buffs.buffs.insert(buff.get_id(), buff);
        //保存自己变身的角色
        if self.base_attr.user_id == from_user {
            self.self_transform_cter = Some(Box::new(self.clone()));
        }
        let mut target_pt = TargetPt::new();
        target_pt
            .target_value
            .push(self.get_map_cell_index() as u32);
        let battle_cter_pt = self.convert_to_battle_cter_pt();
        target_pt.set_transform_cter(battle_cter_pt);
        Ok(target_pt)
    }

    ///角色地图块下标是否有效
    pub fn map_cell_index_is_choiced(&self) -> bool {
        self.index_data.map_cell_index.is_some()
    }

    ///设置角色地图块位置
    pub fn set_map_cell_index(&mut self, index: usize) {
        self.index_data.map_cell_index = Some(index);
    }

    ///获得角色地图块位置
    pub fn get_map_cell_index(&self) -> usize {
        if self.index_data.map_cell_index.is_none() {
            error!(
                "this cter's map_cell_index is None!user_id:{},cter_id:{}",
                self.base_attr.user_id, self.base_attr.cter_id
            );
            return 100;
        }
        self.index_data.map_cell_index.unwrap()
    }

    ///添加道具
    pub fn add_item(&mut self, item_id: u32) -> anyhow::Result<()> {
        let item_temp = TEMPLATES.item_temp_mgr().get_temp(&item_id)?;
        let skill_id = item_temp.trigger_skill;
        let skill_temp = TEMPLATES.skill_temp_mgr().get_temp(&skill_id)?;
        let item = Item {
            id: item_id,
            skill_temp,
        };
        if self.items.len() as u8 >= self.base_attr.item_max {
            anyhow::bail!(
                "this cter's item is full!item_max:{}",
                self.base_attr.item_max
            )
        }
        self.items.insert(item.id, item);
        Ok(())
    }

    pub fn move_index(&mut self, index: usize) {
        self.index_data.last_map_cell_index = Some(self.index_data.map_cell_index.unwrap());
        self.index_data.map_cell_index = Some(index);
    }

    ///消耗buff,如果有buff被删除了，则返回some，否则范围none
    pub fn consume_buff(&mut self, buff_id: u32, is_turn_start: bool) {
        let buff = self.battle_buffs.buffs.get_mut(&buff_id);
        if let Some(buff) = buff {
            if is_turn_start {
                buff.sub_keep_times();
            } else {
                buff.sub_trigger_timesed();
            }
        }
    }

    ///重制角色数据
    pub fn round_reset(&mut self) {
        self.status.is_attacked = false;
        self.change_attack_none();
        self.status.pair_attack_open_count = false;
        self.index_data.map_cell_index = None;
        self.flow_data.open_map_cell_vec.clear();
        self.index_data.last_map_cell_index = None;
        self.flow_data.round_limit_skills.clear();
    }

    pub fn clean_all(&mut self) {
        self.turn_start_reset();
        self.round_reset();
        self.skills.clear();
        self.battle_buffs.buffs.clear();
        self.battle_buffs.passive_buffs.clear();
        self.items.clear();
        self.index_data.map_cell_index = None;
        self.base_attr.element = 0;
        self.battle_buffs.sub_damage_buffs.clear();
        self.battle_buffs.add_damage_buffs.clear();
        self.self_transform_cter = None;
        self.base_attr.grade = 1;
        self.base_attr.hp = 0;
        self.base_attr.atk = 0;
        self.base_attr.defence = 0;
        self.status.state = BattleCterState::Alive;
    }

    ///计算攻击力
    pub fn calc_damage(&self) -> i16 {
        let mut damage = self.base_attr.atk;

        for (buff_id, times) in self.battle_buffs.add_damage_buffs.iter() {
            let buff = self.battle_buffs.buffs.get(buff_id);
            if buff.is_none() {
                continue;
            }
            let buff = buff.unwrap();
            for _ in 0..*times {
                if buff_id == &1001 {
                    damage += buff.buff_temp.par2 as u8;
                } else {
                    damage += buff.buff_temp.par1 as u8;
                }
            }
        }
        damage as i16
    }

    ///计算减伤
    pub fn calc_reduce_damage(&self, attack_is_near: bool) -> i16 {
        let mut value = self.base_attr.defence;
        let mut buff_function_id;
        for (buff_id, times) in self.battle_buffs.sub_damage_buffs.iter() {
            let buff = self.battle_buffs.buffs.get(buff_id);
            if buff.is_none() {
                continue;
            }
            let buff = buff.unwrap();
            buff_function_id = buff.function_id;
            if buff_function_id == NEAR_SUB_ATTACK_DAMAGE && !attack_is_near {
                continue;
            }
            for _ in 0..*times {
                value += buff.buff_temp.par1 as u8;
            }
        }
        value as i16
    }

    ///添加buff
    pub fn add_buff(
        &mut self,
        from_user: Option<u32>,
        from_skill: Option<u32>,
        buff_id: u32,
        turn_index: Option<usize>,
    ) {
        let buff_temp = TEMPLATES.buff_temp_mgr().get_temp(&buff_id);
        if let Err(e) = buff_temp {
            error!("{:?}", e);
            return;
        }
        let buff_temp = buff_temp.unwrap();
        let buff_function_id = buff_temp.function_id;

        let buff = Buff::new(buff_temp, turn_index, from_user, from_skill);

        //增伤
        if ADD_ATTACK.contains(&buff_function_id) {
            self.trigger_add_damage_buff(buff_id);
        }
        //减伤
        if SUB_ATTACK_DAMAGE.contains(&buff_function_id) {
            self.trigger_sub_damage_buff(buff_id);
        }

        self.battle_buffs.buffs.insert(buff.get_id(), buff);
    }

    ///移除buff
    pub fn remove_buff(&mut self, buff_id: u32) {
        self.battle_buffs.buffs.remove(&buff_id);
        self.battle_buffs.add_damage_buffs.remove(&buff_id);
        self.battle_buffs.sub_damage_buffs.remove(&buff_id);
    }

    ///触发增加伤害buff
    pub fn trigger_add_damage_buff(&mut self, buff_id: u32) {
        if !self.battle_buffs.add_damage_buffs.contains_key(&buff_id) {
            self.battle_buffs.add_damage_buffs.insert(buff_id, 1);
        } else {
            let res = self.battle_buffs.add_damage_buffs.get(&buff_id).unwrap();
            let res = *res + 1;
            self.battle_buffs.add_damage_buffs.insert(buff_id, res);
        }
    }

    ///触发减伤buff
    pub fn trigger_sub_damage_buff(&mut self, buff_id: u32) {
        if !self.battle_buffs.sub_damage_buffs.contains_key(&buff_id) {
            self.battle_buffs.sub_damage_buffs.insert(buff_id, 1);
        } else {
            let res = self.battle_buffs.sub_damage_buffs.get(&buff_id).unwrap();
            let res = *res + 1;
            self.battle_buffs.sub_damage_buffs.insert(buff_id, res);
        }
    }

    ///重制翻块次数
    pub fn reset_residue_movement_points(&mut self) {
        self.flow_data.residue_movement_points = TURN_DEFAULT_MOVEMENT_POINTS;
    }

    ///回合开始触发
    pub fn trigger_turn_start(&mut self) {
        let mut buff_function_id;
        for buff in self.battle_buffs.buffs.values() {
            buff_function_id = buff.function_id;
            if CHANGE_SKILL.contains(&buff_function_id) {
                let skill_id = buff.buff_temp.par1;

                let skill_temp = TEMPLATES.skill_temp_mgr().temps.get(&skill_id);
                match skill_temp {
                    None => {
                        error!(
                            "trigger_turn_start the skill_temp can not find!skill_id:{}",
                            skill_id
                        );
                    }
                    Some(st) => {
                        let skill = Skill::from(st);
                        self.skills.remove(&buff.buff_temp.par2);
                        self.skills.insert(skill_id, skill);
                    }
                }
            }
        }
    }

    ///回合结算
    pub fn turn_start_reset(&mut self) {
        //回合开始触发buff
        self.trigger_turn_start();
    }

    pub fn turn_end_reset(&mut self) {
        //重制剩余移动点数
        self.reset_residue_movement_points();
        //重制配对攻击奖励翻地图块次数
        self.status.pair_attack_open_count = false;
        //重制是否翻过地图块
        self.flow_data.open_map_cell_vec.clear();
        //清空turn限制
        self.flow_data.turn_limit_skills.clear();
        //重制是否可以攻击
        self.change_attack_none();
        //重制匹配状态
        self.status.is_pair = false;
        //重制商品购买次数
        self.clear_turn_buy_times();
        //重制任务
        self.reset_mission(MissionResetType::Trun);
        //重制可结束turn状态
        self.set_is_can_end_turn(false);
    }

    ///触发抵挡攻击伤害
    pub fn trigger_attack_damge_gd(&mut self) -> (u32, bool) {
        let gd_buff = self.battle_buffs.get_gd_buff();
        let mut buff_id = 0;
        let mut is_remove = false;
        if gd_buff.is_none() {
            return (buff_id, is_remove);
        }
        let gd_buff = gd_buff.unwrap();

        buff_id = gd_buff.get_id();
        self.consume_buff(buff_id, false);
        let gd_buff = self.battle_buffs.buffs.get_mut(&buff_id).unwrap();
        if gd_buff.trigger_timesed <= 0 || gd_buff.keep_times <= 0 {
            is_remove = true;
        }
        (buff_id, is_remove)
    }

    ///校验角色是否死亡
    pub fn is_died(&self) -> bool {
        self.status.state == BattleCterState::Die
    }

    ///加血
    pub fn add_hp(&mut self, hp: i16) -> bool {
        self.base_attr.hp += hp;
        if self.base_attr.hp <= 0 {
            self.base_attr.hp = 0;
            self.status.state = BattleCterState::Die;
        }
        self.status.state == BattleCterState::Die
    }

    ///将自身转换成protobuf结构体
    pub fn convert_to_battle_cter_pt(&self) -> BattleCharacterPt {
        let mut battle_cter_pt = BattleCharacterPt::new();
        battle_cter_pt.user_id = self.base_attr.user_id;
        battle_cter_pt.cter_id = self.base_attr.cter_id;
        battle_cter_pt.hp = self.base_attr.hp as u32;
        battle_cter_pt.defence = self.base_attr.defence.into();
        battle_cter_pt.atk = self.base_attr.atk as u32;
        battle_cter_pt.energy = self.base_attr.energy as u32;
        battle_cter_pt.index = self.get_map_cell_index() as u32;
        battle_cter_pt.gold = self.gold as u32;
        battle_cter_pt.set_mission(self.mission_data.into_mission_pt());
        self.battle_buffs
            .buffs
            .values()
            .for_each(|buff| battle_cter_pt.buffs.push(buff.get_id()));
        self.skills
            .keys()
            .for_each(|skill_id| battle_cter_pt.skills.push(*skill_id));
        self.items
            .keys()
            .for_each(|item_id| battle_cter_pt.items.push(*item_id));
        battle_cter_pt
    }
}

pub enum TransformInheritValue {
    None,
    Int(usize),
    Other((Vec<usize>, bool)),
}
impl TransformInheritValue {
    pub fn as_usize(&self) -> Option<usize> {
        match self {
            TransformInheritValue::Int(ref n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_other(&self) -> Option<(Vec<usize>, bool)> {
        match self {
            TransformInheritValue::Other(ref n) => Some(n.clone()),
            _ => None,
        }
    }
}

pub struct TransformInherit(TransformInheritType, TransformInheritValue);

pub fn transform_inherit_copy(
    battle_cter: &BattleCharacter,
    target_cter: u32,
) -> Vec<TransformInherit> {
    let target_cter_temp = crate::TEMPLATES
        .character_temp_mgr()
        .get_temp_ref(&target_cter)
        .unwrap();
    let transform_inherit = target_cter_temp.transform_inherit.clone();
    let mut v = vec![];
    for &ti in transform_inherit.iter() {
        let ti_type = TransformInheritType::try_from(ti);
        if let Err(e) = ti_type {
            error!("{:?}", e);
            continue;
        }
        let ti_type = ti_type.unwrap();
        let res = match ti_type {
            TransformInheritType::Hp => {
                TransformInheritValue::Int(battle_cter.base_attr.hp as usize)
            }
            TransformInheritType::Attack => {
                TransformInheritValue::Int(battle_cter.base_attr.atk as usize)
            }
            TransformInheritType::ResidueOpenTimes => {
                TransformInheritValue::Int(battle_cter.flow_data.residue_movement_points as usize)
            }
            TransformInheritType::AttackState => {
                TransformInheritValue::Int(battle_cter.get_attack_state().into_u8() as usize)
            }
            TransformInheritType::MapIndex => {
                TransformInheritValue::Int(battle_cter.index_data.map_cell_index.unwrap())
            }
            TransformInheritType::Energy => {
                TransformInheritValue::Int(battle_cter.base_attr.energy as usize)
            }
            TransformInheritType::Gold => TransformInheritValue::Int(battle_cter.gold as usize),
            TransformInheritType::Buff => {
                TransformInheritValue::Int(battle_cter.base_attr.hp as usize)
            }
            _ => TransformInheritValue::None,
        };
        v.push(TransformInherit(ti_type, res));
    }
    let res = TransformInherit(
        TransformInheritType::Other,
        TransformInheritValue::Other((
            battle_cter.flow_data.open_map_cell_vec.clone(),
            battle_cter.get_is_can_end_turn(),
        )),
    );
    v.push(res);
    v
}
