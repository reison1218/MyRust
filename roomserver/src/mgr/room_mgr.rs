use super::*;

use crate::entity::room::Room;
use crate::entity::room_model::{CustomRoom, MatchRooms, RoomModel, RoomType};
use crate::handlers::room_handler::{
    change_team, choose_character, create_room, emoji, join_room, kick_member, leave_room,
    prepare_cancel, room_setting, search_room, start,
};
use log::{error, warn};
use tools::util::packet::Packet;

//房间服管理器
pub struct RoomMgr {
    pub custom_room: CustomRoom,        //自定义房
    pub match_rooms: MatchRooms,        //公共房
    pub player_room: HashMap<u32, u64>, //玩家对应的房间，key:u32,value:采用一个u64存，通过位运算分出高低位,低32位是房间模式,告32位是房间id
    pub cmd_map: HashMap<u32, fn(&mut RoomMgr, Packet) -> anyhow::Result<()>, RandomState>, //命令管理 key:cmd,value:函数指针
    pub sender: Option<TcpSender>, //tcp channel的发送方
}

impl RoomMgr {
    pub fn new() -> RoomMgr {
        let cmd_map: HashMap<u32, fn(&mut RoomMgr, Packet) -> anyhow::Result<()>, RandomState> =
            HashMap::new();
        let custom_room = CustomRoom::default();
        let match_rooms = MatchRooms::default();
        let player_room: HashMap<u32, u64> = HashMap::new();
        let mut rm = RoomMgr {
            custom_room,
            match_rooms,
            player_room,
            sender: None,
            cmd_map,
        };
        rm.cmd_init();
        rm
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
        let res: anyhow::Result<()> = f.unwrap()(self, packet);
        match res {
            Ok(_) => {}
            Err(_) => {}
        }
    }

    pub fn send(&mut self, bytes: Vec<u8>) {
        if self.sender.is_none() {
            error!("room_mgr'sender is None!");
            return;
        }
        let res = self.sender.as_mut().unwrap().write(bytes);
        if res.is_err() {
            error!("{:?}", res.err().unwrap().to_string());
        }
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
            return self.match_rooms.get_room_mut(&room_id);
        } else if model == RoomType::into_u32(RoomType::SeasonPve) {
            return None;
        }
        None
    }

    ///命令初始化
    fn cmd_init(&mut self) {
        self.cmd_map
            .insert(RoomCode::CreateRoom as u32, create_room);
        self.cmd_map.insert(RoomCode::LeaveRoom as u32, leave_room);
        self.cmd_map
            .insert(RoomCode::ChangeTeam as u32, change_team);
        self.cmd_map.insert(RoomCode::Kick as u32, kick_member);
        self.cmd_map
            .insert(RoomCode::PrepareCancel as u32, prepare_cancel);
        self.cmd_map.insert(RoomCode::LineOff as u32, leave_room);
        self.cmd_map.insert(RoomCode::JoinRoom as u32, join_room);
        self.cmd_map
            .insert(RoomCode::SearchRoom as u32, search_room);
        self.cmd_map
            .insert(RoomCode::RoomSetting as u32, room_setting);
        self.cmd_map
            .insert(RoomCode::ChoiceCharacter as u32, choose_character);
        self.cmd_map.insert(RoomCode::StartGame as u32, start);
        self.cmd_map.insert(RoomCode::Emoji as u32, emoji);
    }
}
