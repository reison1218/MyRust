use super::*;
use async_std::sync::Mutex;
use async_std::task::block_on;
use async_trait::async_trait;
use log::error;
use tools::cmd_code::{ClientCode, RoomCode};

pub enum TcpClientType {
    GameServer,
    RoomServer,
    GameCenter,
}

pub struct TcpClientHandler {
    client_type: TcpClientType,
    ts: Option<TcpStream>,
    cp: Arc<Mutex<ChannelMgr>>,
}

impl TcpClientHandler {
    pub fn new(cp: Arc<Mutex<ChannelMgr>>, client_type: TcpClientType) -> TcpClientHandler {
        let tch = TcpClientHandler {
            ts: None,
            cp,
            client_type,
        };
        tch
    }

    ///数据包转发
    fn arrange_packet(&mut self, packet: Packet) {
        //转发到游戏服
        if packet.get_cmd() >= GameCode::Min as u32 && packet.get_cmd() <= GameCode::Max as u32 {
            let mut lock = block_on(self.cp.lock());
            lock.write_to_game(packet);
            return;
        }
        //转发到房间服
        if packet.get_cmd() >= RoomCode::Min as u32 && packet.get_cmd() <= RoomCode::Max as u32 {
            let mut lock = block_on(self.cp.lock());
            lock.write_to_game_center(packet);
            return;
        }
    }
}

#[async_trait]
impl ClientHandler for TcpClientHandler {
    async fn on_open(&mut self, ts: TcpStream) {
        match self.client_type {
            TcpClientType::GameServer => {
                block_on(self.cp.lock()).set_game_client_channel(ts.try_clone().unwrap());
            }
            // TcpClientType::RoomServer => {
            //     block_on(self.cp.lock()).set_room_client_channel(ts.try_clone().unwrap());
            // }
            TcpClientType::GameCenter => {
                block_on(self.cp.lock()).set_game_center_client_channel(ts.try_clone().unwrap());
            }
        }
        self.ts = Some(ts);
    }

    async fn on_close(&mut self) {
        let address: Option<&str>;
        match self.client_type {
            TcpClientType::GameServer => {
                address = Some(CONF_MAP.get_str("game_port"));
            }
            TcpClientType::RoomServer => {
                address = Some(CONF_MAP.get_str("room_port"));
            }
        }
        self.on_read(address.unwrap().to_string()).await;
    }

    async fn on_message(&mut self, mess: Vec<u8>) {
        let packet_array = Packet::build_array_from_server(mess);

        if let Err(e) = packet_array {
            error!("{:?}", e.to_string());
            return;
        }
        let packet_array = packet_array.unwrap();

        for mut packet in packet_array {
            //判断是否是发给客户端消息
            if packet.is_client() && packet.get_cmd() > 0 {
                let mut lock = block_on(self.cp.lock());
                let gate_user = lock.get_mut_user_channel_channel(&packet.get_user_id());
                match gate_user {
                    Some(user) => {
                        user.get_tcp_mut_ref().write(packet.build_client_bytes());
                        info!(
                            "回给客户端消息,user_id:{},cmd:{}",
                            packet.get_user_id(),
                            packet.get_cmd(),
                        );
                    }
                    None => {
                        if packet.get_cmd() == ClientCode::LeaveRoom.into_u32()
                            || packet.get_cmd() == ClientCode::MemberLeaveNotice.into_u32()
                        {
                            continue;
                        }
                        warn!(
                            "user data is null,id:{},cmd:{}",
                            &packet.get_user_id(),
                            packet.get_cmd()
                        );
                    }
                }
            } else {
                //判断是否要转发到其他服务器进程消息
                self.arrange_packet(packet);
            }
        }
    }
}
