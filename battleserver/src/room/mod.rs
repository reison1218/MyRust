pub mod character;
pub mod map_data;
pub mod member;
pub mod room;

use num_enum::IntoPrimitive;
use num_enum::TryFromPrimitive;
use tools::protos::base::RoomSettingPt;

///最大成员数量
pub const MEMBER_MAX: u8 = 4;

///teamID枚举
#[derive(Debug, Clone, Copy, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum TeamId {
    Min = 1, //最小teamid
    Max = 4, //最大teamid
}

///房间类型
#[derive(Debug, Clone, Copy, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum RoomType {
    None = 0,                  //无效
    OneVOneVOneVOneCustom = 1, //1v1v1v1自定义房间
    OneVOneVOneVOneMatch = 2,  //1v1v1v1匹配房间
    WorldBossCustom = 3,       //世界boss自定义房间
    WorldBoseMatch = 4,        //世界boss匹配房间
}

impl RoomType {
    pub fn into_u8(self) -> u8 {
        let res: u8 = self.into();
        res
    }

    pub fn into_u32(self) -> u32 {
        let res: u8 = self.into();
        res as u32
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum RoomState {
    AwaitConfirm = 0,  //等待进入 只有匹配模式才会有到壮体啊
    Await = 1,         //等待
    ChoiceIndex = 2,   //选择占位
    BattleStarted = 3, //战斗开始
    BattleOvered = 4,  //战斗结束
}

#[derive(Debug, Clone, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum RoomMemberNoticeType {
    None = 0,         //无效
    UpdateMember = 2, //更新成员
}

#[derive(Debug, Clone, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum MemberLeaveNoticeType {
    None = 0,   //无效
    Leave = 1,  //自己离开
    Kicked = 2, //被T
}

///房间设置
#[derive(Debug, Copy, Clone, Default)]
pub struct RoomSetting {
    pub turn_limit_time: u32,   //回合限制时间
    pub season_id: u32,         //赛季id
    pub is_open_ai: bool,       //是否开启ai
    pub victory_condition: u32, //胜利条件
}

impl From<&RoomSettingPt> for RoomSetting {
    fn from(rs_pt: &RoomSettingPt) -> Self {
        let is_open_ai = rs_pt.is_open_ai;
        let victory_condition = rs_pt.victory_condition;
        let turn_limit_time = rs_pt.turn_limit_time;
        let season_id = rs_pt.season_id;
        let rs = RoomSetting {
            turn_limit_time,
            season_id,
            is_open_ai,
            victory_condition,
        };
        rs
    }
}

impl From<RoomSetting> for RoomSettingPt {
    fn from(r: RoomSetting) -> Self {
        let mut rsp = RoomSettingPt::new();
        rsp.set_victory_condition(r.victory_condition);
        rsp.set_season_id(r.season_id);
        rsp.set_turn_limit_time(r.turn_limit_time);
        rsp.set_is_open_ai(r.is_open_ai);
        rsp
    }
}
