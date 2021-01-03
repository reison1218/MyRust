use crate::handlers::room_handler::{
    change_team, choice_skills, choose_character, create_room, emoji, join_room, kick_member,
    leave_room, prepare_cancel, reload_temps, room_setting, search_room, start, update_season,
};
use crate::room::room::Room;
use crate::room::room_model::{CustomRoom, MatchRoom, RoomModel, RoomType};
use crate::task_timer::Task;
use crossbeam::channel::Sender;
use log::warn;
use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use tools::cmd_code::{ClientCode, RoomCode, ServerCommonCode};
use tools::tcp::TcpSender;
use tools::util::packet::Packet;

type CmdFn = HashMap<u32, fn(&mut RoomMgr, Packet) -> anyhow::Result<()>, RandomState>;

///房间服管理器
pub struct RoomMgr {
    pub custom_room: CustomRoom,           //自定义房
    pub match_room: MatchRoom,             //公共房
    pub player_room: HashMap<u32, u64>, //玩家对应的房间，key:u32,value:采用一个u64存，通过位运算分出高低位,低32位是房间模式,高32位是房间id
    pub cmd_map: CmdFn,                 //命令管理 key:cmd,value:函数指针
    sender: Option<TcpSender>,          //tcp channel的发送方
    pub task_sender: Option<Sender<Task>>, //task channel的发送方
}

tools::get_mut_ref!(RoomMgr);

impl RoomMgr {
    pub fn new() -> RoomMgr {
        let cmd_map: HashMap<u32, fn(&mut RoomMgr, Packet) -> anyhow::Result<()>, RandomState> =
            HashMap::new();
        let custom_room = CustomRoom::default();
        let match_rooms = MatchRoom::default();
        let player_room: HashMap<u32, u64> = HashMap::new();
        let mut rm = RoomMgr {
            custom_room,
            match_room: match_rooms,
            player_room,
            sender: None,
            task_sender: None,
            cmd_map,
        };
        rm.cmd_init();
        rm
    }

    pub fn get_task_sender_clone(&self) -> crossbeam::channel::Sender<Task> {
        self.task_sender.as_ref().unwrap().clone()
    }

    pub fn send_2_client(&mut self, cmd: ClientCode, user_id: u32, bytes: Vec<u8>) {
        let bytes = Packet::build_packet_bytes(cmd.into_u32(), user_id, bytes, true, true);
        self.get_sender_mut().send(bytes);
    }

    pub fn set_sender(&mut self, sender: TcpSender) {
        self.sender = Some(sender);
    }

    pub fn get_sender_clone(&self) -> TcpSender {
        self.sender.clone().unwrap()
    }

    pub fn get_sender_mut(&mut self) -> &mut TcpSender {
        self.sender.as_mut().unwrap()
    }

    ///检查玩家是否已经在房间里
    pub fn check_player(&self, user_id: &u32) -> bool {
        self.player_room.contains_key(user_id)
    }

    pub fn get_room_id(&self, user_id: &u32) -> Option<u32> {
        let res = self.player_room.get(user_id);
        if res.is_none() {
            return None;
        }
        let res = res.unwrap();
        let (_, room_id) = tools::binary::separate_long_2_int(*res);
        return Some(room_id);
    }

    ///执行函数，通过packet拿到cmd，然后从cmdmap拿到函数指针调用
    pub fn invok(&mut self, packet: Packet) {
        let cmd = packet.get_cmd();
        let f = self.cmd_map.get_mut(&cmd);
        if f.is_none() {
            warn!("there is no handler of cmd:{:?}!", cmd);
            return;
        }
        let _ = f.unwrap()(self, packet);
    }

    pub fn get_room_mut(&mut self, user_id: &u32) -> Option<&mut Room> {
        let res = self.player_room.get(user_id);
        if res.is_none() {
            return None;
        }
        let res = res.unwrap();
        let (model, room_id) = tools::binary::separate_long_2_int(*res);

        if model == RoomType::into_u32(RoomType::Custom) {
            return self.custom_room.get_room_mut(&room_id);
        } else if model == RoomType::into_u32(RoomType::Match) {
            return self.match_room.get_room_mut(&room_id);
        } else if model == RoomType::into_u32(RoomType::SeasonPve) {
            return None;
        }
        None
    }

    #[allow(dead_code)]
    pub fn get_room_ref(&self, user_id: &u32) -> Option<&Room> {
        let res = self.player_room.get(user_id);
        if res.is_none() {
            return None;
        }
        let res = res.unwrap();
        let (model, room_id) = tools::binary::separate_long_2_int(*res);

        if model == RoomType::into_u32(RoomType::Custom) {
            return self.custom_room.get_room_ref(&room_id);
        } else if model == RoomType::into_u32(RoomType::Match) {
            return self.match_room.get_room_ref(&room_id);
        } else if model == RoomType::into_u32(RoomType::SeasonPve) {
            return None;
        }
        None
    }

    ///删除房间
    pub fn rm_room(&mut self, room_id: u32, room_type: RoomType, member_v: Vec<u32>) {
        match room_type {
            RoomType::Match => {
                self.match_room.rm_room(&room_id);
            }
            RoomType::Custom => {
                self.custom_room.rm_room(&room_id);
            }
            _ => {}
        }
        for user_id in member_v {
            self.player_room.remove(&user_id);
        }
    }

    ///命令初始化
    fn cmd_init(&mut self) {
        //更新赛季信息
        self.cmd_map
            .insert(ServerCommonCode::UpdateSeason.into_u32(), update_season);
        //热更静态配置
        self.cmd_map
            .insert(ServerCommonCode::ReloadTemps.into_u32(), reload_temps);
        //离开房间
        self.cmd_map
            .insert(ServerCommonCode::LeaveRoom.into_u32(), leave_room);
        //离线
        self.cmd_map
            .insert(ServerCommonCode::LineOff.into_u32(), leave_room);
        //创建房间
        self.cmd_map
            .insert(RoomCode::CreateRoom.into_u32(), create_room);
        //换队伍
        self.cmd_map
            .insert(RoomCode::ChangeTeam.into_u32(), change_team);
        //T人
        self.cmd_map.insert(RoomCode::Kick.into_u32(), kick_member);
        //准备与取消
        self.cmd_map
            .insert(RoomCode::PrepareCancel.into_u32(), prepare_cancel);

        //添加房间
        self.cmd_map
            .insert(RoomCode::JoinRoom.into_u32(), join_room);
        //匹配房间
        self.cmd_map
            .insert(RoomCode::SearchRoom.into_u32(), search_room);
        //房间设置
        self.cmd_map
            .insert(RoomCode::RoomSetting.into_u32(), room_setting);
        //选择角色
        self.cmd_map
            .insert(RoomCode::ChoiceCharacter.into_u32(), choose_character);
        //选择技能
        self.cmd_map
            .insert(RoomCode::ChoiceSkill.into_u32(), choice_skills);
        //发送表情
        self.cmd_map.insert(RoomCode::Emoji.into_u32(), emoji);
        //开始游戏
        self.cmd_map.insert(RoomCode::StartGame.into_u32(), start);
    }
}
