use crate::battle::battle::BattleData;
use crate::battle::battle_enum::skill_judge_type::{
    HP_LIMIT_GT, LIMIT_ROUND_TIMES, LIMIT_TURN_TIMES,
};
use crate::battle::battle_enum::{
    AttackState, BattleCterState, EffectType, TargetType, TRIGGER_SCOPE_NEAR_TEMP_ID,
};
use crate::battle::battle_trigger::TriggerEvent;
use crate::room::character::BattleCharacter;
use crate::room::map_data::{Cell, CellType, TileMap};
use crate::room::room::MEMBER_MAX;
use crate::task_timer::{Task, TaskCmd};
use crate::TEMPLATES;
use log::{error, info, warn};
use protobuf::Message;
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::convert::TryFrom;
use tools::cmd_code::ClientCode;
use tools::protos::base::{ActionUnitPt, BuffPt, CellBuffPt, EffectPt, TargetPt, TriggerEffectPt};
use tools::protos::battle::S_BATTLE_TURN_NOTICE;
use tools::templates::skill_scope_temp::SkillScopeTemp;
use tools::util::packet::Packet;

impl BattleData {
    ///检测地图刷新
    pub fn check_refresh_map(&mut self) -> bool {
        let allive_count = self
            .battle_cter
            .values()
            .filter(|x| x.state == BattleCterState::Alive)
            .count();

        let un_open_count = self.tile_map.un_pair_map.len();
        let mut need_reflash_map = false;
        if un_open_count <= 2 {
            need_reflash_map = true;
        }
        if allive_count >= 2 && need_reflash_map {
            return true;
        }
        false
    }

    ///下一个
    pub fn add_next_turn_index(&mut self) {
        self.next_turn_index += 1;
        self.add_total_turn_times();
        let index = self.next_turn_index;
        if index >= MEMBER_MAX as usize {
            self.next_turn_index = 0;
        }
        //开始回合触发
        self.turn_start_summary();

        let user_id = self.get_turn_user(None);
        if let Ok(user_id) = user_id {
            if user_id == 0 {
                self.add_next_turn_index();
                return;
            }

            let cter_res = self.get_battle_cter(Some(user_id), false);
            match cter_res {
                Ok(cter) if cter.is_died() => {
                    self.add_next_turn_index();
                    return;
                }
                Err(e) => {
                    warn!("{:?}", e);
                }
                _ => {}
            }
        } else {
            warn!("{:?}", user_id.err().unwrap());
        }
    }

    ///处理角色移动之后的事件
    pub unsafe fn handler_cter_move(
        &mut self,
        user_id: u32,
        index: usize,
        au: &mut ActionUnitPt,
    ) -> anyhow::Result<Vec<ActionUnitPt>> {
        let battle_cters = &mut self.battle_cter as *mut HashMap<u32, BattleCharacter>;
        let battle_cter = battle_cters.as_mut().unwrap().get_mut(&user_id).unwrap();
        let tile_map = self.tile_map.borrow_mut() as *mut TileMap;
        let cell = tile_map.as_mut().unwrap().map.get_mut(index).unwrap();
        au.action_value.push(cell.id);
        let last_index = battle_cter.get_cell_index();
        let mut is_change_index_both = false;
        let tile_map_ptr = self.tile_map.borrow_mut() as *mut TileMap;
        //判断改地图块上面有没有角色，有的话将目标位置的玩家挪到操作玩家的位置上
        if cell.user_id > 0 {
            let target_user = cell.user_id;
            //先判断目标位置的角色是否有不动泰山被动技能
            self.before_moved_trigger(user_id, target_user)?;
            let target_cter = self.get_battle_cter_mut(Some(target_user), true).unwrap();
            target_cter.move_index(battle_cter.get_cell_index());

            let source_cell = tile_map_ptr
                .as_mut()
                .unwrap()
                .map
                .get_mut(last_index)
                .unwrap();
            source_cell.user_id = target_cter.user_id;
            is_change_index_both = true;
        } else {
            //重制之前地图块上的玩家id
            let last_cell = tile_map_ptr
                .as_mut()
                .unwrap()
                .map
                .get_mut(last_index)
                .unwrap();
            last_cell.user_id = 0;
        }
        //改变角色位置
        battle_cter.move_index(index);
        cell.user_id = battle_cter.user_id;

        let index = index as isize;
        //移动位置后触发事件
        let v = self.after_move_trigger(
            battle_cter,
            index,
            is_change_index_both,
            battle_cters.as_mut().unwrap(),
        );
        Ok(v)
    }

    ///消耗buff
    pub unsafe fn consume_buff(
        &mut self,
        buff_id: u32,
        user_id: Option<u32>,
        cell_index: Option<usize>,
        is_turn_index: bool,
    ) -> Option<u32> {
        let next_turn_index = self.next_turn_index;
        let mut cter_res: Option<&mut BattleCharacter> = None;
        let mut cell_res: Option<&mut Cell> = None;
        let mut lost_buff = None;
        let cters = self.battle_cter.borrow_mut() as *mut HashMap<u32, BattleCharacter>;
        if user_id.is_some() {
            let user_id = user_id.unwrap();
            let cter = self.get_battle_cter_mut(Some(user_id), true);
            if let Err(e) = cter {
                error!("{:?}", e);
                return lost_buff;
            }
            let cter = cter.unwrap();
            let buff = cter.buffs.get_mut(&buff_id);
            if buff.is_none() {
                return lost_buff;
            }
            cter_res = Some(cter);
        } else if cell_index.is_some() {
            let cell_index = cell_index.unwrap();
            let cell = self.tile_map.map.get_mut(cell_index);
            if cell.is_none() {
                return lost_buff;
            }
            let cell = cell.unwrap();
            let buff = cell.buffs.get_mut(&buff_id);
            if buff.is_none() {
                return lost_buff;
            }
            cell_res = Some(cell);
        }

        let buff;
        if cter_res.is_some() {
            buff = cter_res.as_mut().unwrap().buffs.get_mut(&buff_id);
        } else if cell_res.is_some() {
            buff = cell_res.as_mut().unwrap().buffs.get_mut(&buff_id);
        } else {
            return lost_buff;
        }
        let buff = buff.unwrap();

        let need_remove;
        if is_turn_index && buff.turn_index.is_some() && buff.turn_index.unwrap() == next_turn_index
        {
            buff.sub_keep_times();
        } else {
            buff.sub_trigger_timesed()
        }
        if buff.keep_times <= 0 || buff.trigger_timesed <= 0 {
            need_remove = true;
        } else {
            need_remove = false;
        }

        if need_remove {
            if buff.from_user.is_some() {
                let from_user = buff.from_user.unwrap();
                let from_cter = cters.as_mut().unwrap().get_mut(&from_user);
                if from_cter.is_none() {
                    error!("can not find battle_cter!user_id:{}", from_user);
                    return lost_buff;
                }
                let from_cter = from_cter.unwrap();
                if buff.from_skill.is_none() {
                    return lost_buff;
                }
                let from_skill = buff.from_skill.unwrap();
                let skill = from_cter.skills.get_mut(&from_skill);
                if skill.is_none() {
                    return lost_buff;
                }
                let skill = skill.unwrap();
                skill.is_active = false;
            }

            //如果是玩家身上的
            if let Some(cter) = cter_res {
                cter.remove_buff(buff_id);
                lost_buff = Some(buff_id);
                let user_id = cter.user_id;
                self.buff_lost_trigger(user_id, buff_id);
            } else if let Some(cell) = cell_res {
                //如果是地图块上面的
                cell.remove_buff(buff_id);
            }
        }
        lost_buff
    }

    ///加血
    pub fn add_hp(
        &mut self,
        from_user: Option<u32>,
        target: u32,
        hp: i16,
        buff_id: Option<u32>,
    ) -> anyhow::Result<TargetPt> {
        let cter = self.get_battle_cter_mut(Some(target), true)?;

        if cter.is_died() {
            anyhow::bail!(
                "this cter is died! user_id:{},cter_id:{}",
                target,
                cter.cter_id
            )
        }
        cter.add_hp(hp);
        let target_pt =
            self.build_target_pt(from_user, target, EffectType::Cure, hp as u32, buff_id)?;
        Ok(target_pt)
    }

    ///计算减伤
    pub fn calc_reduce_damage(&self, from_user: u32, target_cter: &mut BattleCharacter) -> i16 {
        let target_user = target_cter.user_id;
        let target_index = target_cter.get_cell_index() as isize;
        let scope_temp = TEMPLATES
            .get_skill_scope_ref()
            .get_temp(&TRIGGER_SCOPE_NEAR_TEMP_ID);
        if let Err(_) = scope_temp {
            return target_cter.defence as i16;
        }
        let scope_temp = scope_temp.unwrap();
        let (_, user_v) = self.cal_scope(
            target_user,
            target_index,
            TargetType::None,
            None,
            Some(scope_temp),
        );
        let res = user_v.contains(&from_user);
        target_cter.calc_reduce_damage(res)
    }

    ///扣血
    pub unsafe fn deduct_hp(
        &mut self,
        from: u32,
        target: u32,
        skill_damege: Option<i16>,
        need_rank: bool,
    ) -> anyhow::Result<TargetPt> {
        let battle_data_ptr = self as *mut BattleData;
        let mut target_pt = TargetPt::new();

        let mut ep = EffectPt::new();
        ep.effect_type = EffectType::SkillDamage as u32;

        let target_cter = battle_data_ptr
            .as_mut()
            .unwrap()
            .get_battle_cter_mut(Some(target), true)?;
        target_pt
            .target_value
            .push(target_cter.get_cell_index() as u32);
        let mut res;
        //如果是普通攻击，要算上减伤
        if skill_damege.is_none() {
            let from_cter = battle_data_ptr
                .as_mut()
                .unwrap()
                .get_battle_cter_mut(Some(from), true)?;
            let attack_damage = from_cter.calc_damage();
            let reduce_damage = self.calc_reduce_damage(from_cter.user_id, target_cter);
            ep.effect_type = EffectType::AttackDamage as u32;
            res = attack_damage - reduce_damage;
            if res < 0 {
                res = 0;
            }
            let gd_buff = target_cter.trigger_attack_damge_gd();
            if gd_buff.0 > 0 {
                let mut te_pt = TriggerEffectPt::new();
                te_pt.set_buff_id(gd_buff.0);
                target_pt.passiveEffect.push(te_pt);
                if gd_buff.1 {
                    let lost_buff =
                        self.consume_buff(gd_buff.0, Some(target_cter.user_id), None, false);
                    if let Some(lost_buff) = lost_buff {
                        target_pt.lost_buffs.push(lost_buff);
                    }
                }
                res = 0;
            } else {
                target_cter.is_attacked = true;
            }
        } else {
            res = skill_damege.unwrap();
        }
        ep.effect_value = res as u32;
        target_pt.effects.push(ep);
        let is_die = target_cter.sub_hp(res);

        //判断目标角色是否死亡
        if is_die {
            //判断是否需要排行
            if need_rank {
                self.rank_vec.push(Vec::new());
            }
            //此处做一个容错处理
            if self.rank_vec.is_empty() {
                self.rank_vec.push(Vec::new());
            }
            let mut rank_vec_size = self.rank_vec.len();
            if rank_vec_size != 0 {
                rank_vec_size -= 1;
            }
            let v = self.rank_vec.get_mut(rank_vec_size);
            if v.is_none() {
                error!("rank_vec can not find data!rank_vec_size:{}", rank_vec_size);
                return Ok(target_pt);
            }
            v.unwrap().push(target);

            let cell = self.tile_map.get_cell_mut_by_user_id(target);
            if let Some(cell) = cell {
                cell.user_id = 0;
            }
        }
        Ok(target_pt)
    }

    ///处理地图块配对逻辑
    pub unsafe fn handler_cell_pair(&mut self, user_id: u32) -> bool {
        let battle_cters = &mut self.battle_cter as *mut HashMap<u32, BattleCharacter>;

        let battle_cter = battle_cters.as_mut().unwrap().get_mut(&user_id);
        if let None = battle_cter {
            error!("cter is not find!user_id:{}", user_id);
            return false;
        }
        let battle_cter = battle_cter.unwrap();

        let index = battle_cter.get_cell_index();
        let cell = self.tile_map.map.get_mut(index);
        if let None = cell {
            error!("cell is not find!cell_index:{}", index);
            return false;
        }
        let cell_ptr = cell.unwrap() as *mut Cell;
        let cell_mut = cell_ptr.as_mut().unwrap();
        let mut is_pair = false;
        let cell_id = cell_mut.id;
        if battle_cter.open_cell_vec.is_empty() || battle_cter.is_pair {
            return is_pair;
        }
        let size = battle_cter.open_cell_vec.len();
        let last_open_cell_index = *battle_cter.open_cell_vec.get(size - 1).unwrap();
        let res = self.tile_map.map.get_mut(last_open_cell_index);
        if let None = res {
            error!("cell not find!cell_index:{}", last_open_cell_index);
            return false;
        }
        let last_cell = res.unwrap() as *mut Cell;
        self.tile_map.map.get_mut(last_open_cell_index);
        let last_cell_id: Option<u32> = Some(last_cell.as_ref().unwrap().id);
        let last_cell = &mut *last_cell;
        //如果配对了，则修改地图块配对的下标
        if let Some(id) = last_cell_id {
            if cell_id == id {
                cell_mut.pair_index = Some(last_open_cell_index);
                last_cell.pair_index = Some(index);
                is_pair = true;
                battle_cter.is_pair = true;
                //状态改为可以进行攻击
                if battle_cter.attack_state != AttackState::Locked {
                    battle_cter.attack_state = AttackState::Able;
                }
                self.tile_map.un_pair_map.remove(&last_cell.index);
                self.tile_map.un_pair_map.remove(&cell_mut.index);
            }
        } else {
            is_pair = false;
        }
        //配对了就封装
        if is_pair {
            info!(
                "user:{} open cell pair! last_cell:{},now_cell:{}",
                battle_cter.user_id, last_open_cell_index, index
            );
        }
        is_pair
    }
    ///发送战斗turn推送
    pub fn send_battle_turn_notice(&mut self) {
        let mut sbtn = S_BATTLE_TURN_NOTICE::new();
        sbtn.set_user_id(self.get_turn_user(None).unwrap());
        //角色身上的
        for cter in self.battle_cter.values() {
            let cter_pt = cter.convert_to_battle_cter();
            sbtn.cters.push(cter_pt);
        }

        //地图块身上的
        for cell in self.tile_map.map.iter() {
            let mut cbp = CellBuffPt::new();
            cbp.index = cell.index as u32;
            for buff in cell.buffs.values() {
                if cell.passive_buffs.contains(&buff.id) {
                    continue;
                }
                let mut buff_pt = BuffPt::new();
                buff_pt.buff_id = buff.id;
                buff_pt.trigger_timesed = buff.trigger_timesed as u32;
                buff_pt.keep_times = buff.keep_times as u32;
                cbp.buffs.push(buff_pt);
            }
            sbtn.cell_buffs.push(cbp);
        }

        let bytes = sbtn.write_to_bytes().unwrap();
        for user_id in self.battle_cter.clone().keys() {
            self.send_2_client(ClientCode::BattleTurnNotice, *user_id, bytes.clone());
        }
    }
    ///获得战斗角色可变借用指针
    pub fn get_battle_cter_mut(
        &mut self,
        user_id: Option<u32>,
        is_alive: bool,
    ) -> anyhow::Result<&mut BattleCharacter> {
        let _user_id;
        if let Some(user_id) = user_id {
            _user_id = user_id;
        } else {
            let res = self.get_turn_user(None);
            if let Err(e) = res {
                anyhow::bail!("{:?}", e)
            }
            _user_id = res.unwrap();
        }
        let cter = self.battle_cter.get_mut(&_user_id);
        if let None = cter {
            anyhow::bail!("battle_cter not find!user_id:{}", _user_id)
        }
        let cter = cter.unwrap();
        if is_alive && cter.is_died() {
            anyhow::bail!(
                "this battle_cter is already died!user_id:{},cter_id:{}",
                _user_id,
                cter.cter_id
            )
        }
        Ok(cter)
    }

    pub fn send_2_client(&mut self, cmd: ClientCode, user_id: u32, bytes: Vec<u8>) {
        let bytes = Packet::build_packet_bytes(cmd as u32, user_id, bytes, true, true);
        self.get_sender_mut().write(bytes);
    }

    ///检查目标数组
    pub fn check_target_array(
        &self,
        user_id: u32,
        target_type: TargetType,
        target_array: &[u32],
    ) -> anyhow::Result<()> {
        match target_type {
            //无效目标
            TargetType::None => {
                anyhow::bail!("this target_type is invaild!target_type:{:?}", target_type)
            }
            //任意玩家
            TargetType::AnyPlayer => {
                let mut v = Vec::new();
                for index in target_array {
                    let cter = self.get_battle_cter_by_cell_index(*index as usize)?;
                    v.push(cter.user_id);
                    break;
                }
                self.check_user_target(&v[..], None)?; //不包括自己的其他玩家
            } //玩家自己
            TargetType::PlayerSelf => {
                if target_array.len() > 1 {
                    anyhow::bail!("this target_type is invaild!target_type:{:?}", target_type)
                }
                for index in target_array {
                    let cter = self.get_battle_cter_by_cell_index(*index as usize)?;
                    if cter.user_id != user_id {
                        anyhow::bail!("this target_type is invaild!target_type:{:?}", target_type)
                    }
                }
            } //玩家自己
            //全图玩家
            TargetType::AllPlayer => {
                let mut v = Vec::new();
                for index in target_array {
                    let cter = self.get_battle_cter_by_cell_index(*index as usize)?;
                    v.push(cter.user_id);
                }
                self.check_user_target(&v[..], None)?; //不包括自己的其他玩家
            }
            TargetType::OtherAllPlayer => {
                let mut v = Vec::new();
                for index in target_array {
                    let cter = self.get_battle_cter_by_cell_index(*index as usize)?;
                    v.push(cter.user_id);
                }
                //除自己所有玩家
                self.check_user_target(&v[..], Some(user_id))?
            } //除自己外任意玩家
            TargetType::OtherAnyPlayer => {
                let mut v = Vec::new();
                for index in target_array {
                    let cter = self.get_battle_cter_by_cell_index(*index as usize)?;
                    v.push(cter.user_id);
                    break;
                }
                //除自己所有玩家
                self.check_user_target(&v[..], Some(user_id))?
            }
            TargetType::SelfScopeOthers => {
                let mut v = Vec::new();
                for index in target_array {
                    let cter = self.get_battle_cter_by_cell_index(*index as usize)?;
                    v.push(cter.user_id);
                    break;
                }
                //除自己所有玩家
                self.check_user_target(&v[..], Some(user_id))?
            }
            //地图块
            TargetType::Cell => {
                //校验地图块下标有效性
                for index in target_array {
                    let index = *index as usize;
                    self.check_choice_index(index, false, false, false, false)?;
                }
            }
            //未翻开的地图块
            TargetType::UnOpenCell => {
                for index in target_array {
                    self.check_choice_index(*index as usize, true, true, true, false)?;
                }
            } //未配对的地图块
            TargetType::UnPairCell => {
                for index in target_array {
                    self.check_choice_index(*index as usize, true, true, true, false)?;
                }
            } //空的地图块
            TargetType::NullCell => {
                for index in target_array {
                    self.check_choice_index(*index as usize, true, true, false, true)?;
                }
            } //空的地图块，上面没人
            TargetType::UnPairNullCell => {
                for index in target_array {
                    let index = *index as usize;
                    self.check_choice_index(index, false, false, false, true)?;
                }
            }
            TargetType::OpenedCell => {
                for index in target_array {
                    let index = *index as usize;
                    self.check_choice_index(index, false, true, false, false)?;
                }
            }
            //其他目标类型
            _ => {}
        }
        Ok(())
    }

    ///检测目标玩家
    pub fn check_user_target(&self, vec: &[u32], check_self_id: Option<u32>) -> anyhow::Result<()> {
        for member_id in vec.iter() {
            let member_id = *member_id;
            //校验有没有
            if !self.battle_cter.contains_key(&member_id) {
                anyhow::bail!("battle_cter is not find!user_id:{}", member_id)
            }
            //校验是不是自己
            if check_self_id.is_some() && member_id == check_self_id.unwrap() {
                anyhow::bail!("target_user_id==self!target_user_id:{}", member_id)
            }
        }
        Ok(())
    }

    //检测地图块是否选择
    pub fn check_choice_index(
        &self,
        index: usize,
        is_check_pair: bool,
        is_check_world: bool,
        is_check_locked: bool,
        is_check_has_user: bool,
    ) -> anyhow::Result<()> {
        let res = self.tile_map.map.get(index);
        if res.is_none() {
            anyhow::bail!("this cell is not find!index:{}", index)
        }
        let cell = res.unwrap();

        if cell.id < CellType::Valid.into_u32() {
            anyhow::bail!("this is cell can not be choice!index:{}", cell.index)
        }

        let cell = res.unwrap();
        if is_check_pair && cell.pair_index.is_some() {
            anyhow::bail!("this cell already pair!index:{}", cell.index)
        }
        if is_check_world && cell.is_world {
            anyhow::bail!("world_cell can not be choice!index:{}", cell.index)
        }
        if is_check_locked && cell.check_is_locked() {
            anyhow::bail!("this cell is locked!index:{}", cell.index)
        }
        if is_check_has_user && cell.user_id > 0 {
            anyhow::bail!("this cell has user!index:{}", cell.index)
        }
        Ok(())
    }

    ///新建战斗回合定时器任务
    pub fn build_battle_turn_task(&self) {
        let next_turn_index = self.next_turn_index;
        let user_id = self.turn_orders.get(next_turn_index);
        if user_id.is_none() {
            error!(
                "user_id is none!next_turn_index:{},turn_orders:{:?}",
                next_turn_index, self.turn_orders
            );
            return;
        }
        let user_id = user_id.unwrap();
        let time_limit = self.turn_limit_time;
        let mut task = Task::default();
        task.delay = time_limit;
        task.cmd = TaskCmd::BattleTurnTime as u16;

        let mut map = serde_json::Map::new();
        map.insert("user_id".to_owned(), serde_json::Value::from(*user_id));
        task.data = serde_json::Value::from(map);
        let res = self.task_sender.send(task);
        if res.is_err() {
            error!("{:?}", res.err().unwrap());
        }
    }

    ///构建targetpt
    pub fn build_target_pt(
        &self,
        from_user: Option<u32>,
        target_user: u32,
        effect_type: EffectType,
        effect_value: u32,
        buff_id: Option<u32>,
    ) -> anyhow::Result<TargetPt> {
        let target_cter = self.get_battle_cter(Some(target_user), true)?;
        let mut target_pt = TargetPt::new();
        target_pt
            .target_value
            .push(target_cter.get_cell_index() as u32);
        if from_user.is_some() && from_user.unwrap() == target_user && buff_id.is_some() {
            let mut tep = TriggerEffectPt::new();
            tep.set_field_type(effect_type.into_u32());
            tep.set_value(effect_value);
            tep.buff_id = buff_id.unwrap();
            target_pt.passiveEffect.push(tep);
        } else {
            let mut ep = EffectPt::new();
            ep.effect_type = effect_type.into_u32();
            ep.effect_value = effect_value;
            target_pt.effects.push(ep);
        }
        Ok(target_pt)
    }

    ///计算范围,返回一个元组类型，前面一个是范围，后面一个是范围内的合法玩家
    /// 当targets和scope_temp为None时,以⭕️为校验范围有没有人
    /// 当targets为None,scope_temp为Some则校验scope_temp范围内有没有人
    /// 当targets和scope_temp都不为None时，校验targets是否在scope_temp范围内
    pub fn cal_scope(
        &self,
        user_id: u32,
        center_index: isize,
        target_type: TargetType,
        targets: Option<Vec<u32>>,
        scope_temp: Option<&SkillScopeTemp>,
    ) -> (Vec<usize>, Vec<u32>) {
        let mut v_u = Vec::new();
        let mut v = Vec::new();
        let center_cell = self.tile_map.map.get(center_index as usize).unwrap();
        //没有目标，只有范围
        if targets.is_none() && scope_temp.is_some() {
            let scope_temp = scope_temp.unwrap();

            for direction_temp2d in scope_temp.scope2d.iter() {
                for coord_temp in direction_temp2d.direction2d.iter() {
                    let x = center_cell.x + coord_temp.x;
                    let y = center_cell.y + coord_temp.y;
                    let cell_index = self.tile_map.coord_map.get(&(x, y));
                    if let None = cell_index {
                        continue;
                    }
                    let cell_index = cell_index.unwrap();
                    let cell = self.tile_map.map.get(*cell_index);
                    if cell.is_none() {
                        continue;
                    }
                    v.push(*cell_index);
                    let cell = cell.unwrap();
                    if cell.user_id <= 0 {
                        continue;
                    }
                    //如果目标不能是自己，就跳过
                    if (target_type == TargetType::OtherAllPlayer
                        || target_type == TargetType::SelfScopeOthers
                        || target_type == TargetType::SelfScopeAnyOthers
                        || target_type == TargetType::OtherAnyPlayer)
                        && cell.user_id == user_id
                    {
                        continue;
                    }
                    let other_user = cell.user_id;
                    //如果玩家id大于0
                    if other_user == 0 {
                        continue;
                    }

                    let cter = self.get_battle_cter(Some(other_user), true);
                    if let Err(e) = cter {
                        warn!("{:?}", e);
                        continue;
                    }
                    v_u.push(other_user);
                }
            }
        } else {
            //两者都有
            let targets = targets.unwrap();
            let scope_temp = scope_temp.unwrap();
            //否则校验选中的区域
            for dir in scope_temp.scope2d.iter() {
                for coord_temp in dir.direction2d.iter() {
                    let x = center_cell.x + coord_temp.x;
                    let y = center_cell.y + coord_temp.y;
                    let cell_index = self.tile_map.coord_map.get(&(x, y));
                    if let None = cell_index {
                        continue;
                    }
                    let cell_index = cell_index.unwrap();
                    let cell = self.tile_map.map.get(*cell_index);
                    if let None = cell {
                        continue;
                    }
                    v.push(*cell_index);
                    let cell = cell.unwrap();
                    for index in targets.iter() {
                        if cell.index as u32 != *index {
                            continue;
                        }
                        let other_user = cell.user_id;
                        //如果目标不能是自己，就跳过
                        if (target_type == TargetType::OtherAllPlayer
                            || target_type == TargetType::SelfScopeOthers
                            || target_type == TargetType::SelfScopeAnyOthers
                            || target_type == TargetType::OtherAnyPlayer)
                            && cell.user_id == user_id
                        {
                            continue;
                        }
                        //如果玩家id大于0
                        if other_user == 0 {
                            continue;
                        }
                        let cter = self.get_battle_cter(Some(other_user), true);
                        if let Err(e) = cter {
                            warn!("{:?}", e);
                            continue;
                        }
                        let cter = cter.unwrap();
                        if v_u.contains(&cter.user_id) {
                            continue;
                        }
                        v_u.push(cter.user_id);
                    }
                }
            }
        }
        (v, v_u)
    }

    ///校验技能条件
    pub fn check_skill_judge(
        &self,
        user_id: u32,
        skill_judge: u32,
        skill_id: Option<u32>,
        _: Option<Vec<u32>>,
    ) -> anyhow::Result<()> {
        if skill_judge == 0 {
            return Ok(());
        }
        let judge_temp = TEMPLATES.get_skill_judge_ref().get_temp(&skill_judge)?;
        let target_type = TargetType::try_from(judge_temp.target);
        if let Err(e) = target_type {
            anyhow::bail!("{:?}", e)
        }
        let cter = self.get_battle_cter(Some(user_id), true).unwrap();
        let target_type = target_type.unwrap();

        match target_type {
            TargetType::PlayerSelf => {
                if HP_LIMIT_GT == judge_temp.id && cter.hp <= judge_temp.par1 as i16 {
                    anyhow::bail!(
                        "HP_LIMIT_GT!hp of cter <= {}!skill_judge_id:{}",
                        judge_temp.par1,
                        judge_temp.id
                    )
                } else if LIMIT_TURN_TIMES == judge_temp.id
                    && cter.turn_limit_skills.contains(&skill_id.unwrap())
                {
                    anyhow::bail!(
                        "this turn already used this skill!cter_id:{},skill_id:{},skill_judge_id:{}",
                        cter.cter_id,
                        skill_id.unwrap(),
                        skill_judge,
                    )
                } else if LIMIT_ROUND_TIMES == judge_temp.id
                    && cter.round_limit_skills.contains(&skill_id.unwrap())
                {
                    anyhow::bail!(
                        "this round already used this skill!cter_id:{},skill_id:{},skill_judge_id:{}",
                        cter.cter_id,
                        skill_id.unwrap(),
                        skill_judge,
                    )
                }
            }
            _ => {}
        }
        Ok(())
    }
}
