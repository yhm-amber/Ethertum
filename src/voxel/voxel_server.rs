
use std::sync::{Arc, RwLock};
use bevy::{prelude::*, tasks::AsyncComputeTaskPool, utils::{HashMap, HashSet}};
use bevy_renet::renet::RenetServer;
use bevy_xpbd_3d::components::RigidBody;

use crate::{game_server::ServerInfo, net::{CellData, SPacket, RenetServerHelper}, util::iter};
use super::{Chunk, ChunkComponent, ChunkPtr, ChunkSystem, MpscRx, MpscTx, WorldGen};


type ChunkLoadingData = (IVec3, ChunkPtr);


pub struct ServerVoxelPlugin;

impl Plugin for ServerVoxelPlugin {
    fn build(&self, app: &mut App) {
        
        app.insert_resource(ServerChunkSystem::new());

        {
            let (tx, rx) = crate::channel_impl::unbounded::<ChunkLoadingData>();
            app.insert_resource(MpscTx(tx));
            app.insert_resource(MpscRx(rx));
        }

        app.add_systems(Update, chunks_load);

    }
}

fn in_load_distance(mid_cp: IVec3, cp: IVec3, vd: IVec2) -> bool {
    (mid_cp.x - cp.x).abs() <= vd.x * Chunk::SIZE &&
    (mid_cp.z - cp.z).abs() <= vd.x * Chunk::SIZE &&
    (mid_cp.y - cp.y).abs() <= vd.y * Chunk::SIZE
}

fn chunks_load(
    mut chunk_sys: ResMut<ServerChunkSystem>,
    mut net_server: ResMut<RenetServer>,
    mut server: ResMut<ServerInfo>,
    mut cmds: Commands,

    mut chunks_loading: Local<HashSet<IVec3>>,  // for detect/skip if is loading
    tx_chunks_loading: Res<MpscTx<ChunkLoadingData>>,
    rx_chunks_loading: Res<MpscRx<ChunkLoadingData>>,
) {
    // todo
    // 优化: 仅当某玩家 进入/退出 移动过区块边界时，才针对更新
    // 待改进: 这里可能有多种加载方法，包括Inner-Outer近距离优先加载，填充IVec3待加载列表并排序方法

    for player in server.online_players.values() {
        let vd = player.chunks_load_distance;
        let cp = Chunk::as_chunkpos(player.position.as_ivec3());

        iter::iter_aabb(vd.x, vd.y, |rp| {
            let chunkpos = rp * Chunk::SIZE + cp;

            if chunks_loading.len() > 8 {  // max_concurrent_loading_chunks
                return;
            }
            if chunk_sys.has_chunk(chunkpos) || chunks_loading.contains(&chunkpos) {
                return;
            }

            let tx = tx_chunks_loading.clone();
            let task = AsyncComputeTaskPool::get().spawn(async move {
                // info!("Load Chunk: {:?}", chunkpos);
                let mut chunk = Chunk::new(chunkpos);

                WorldGen::generate_chunk(&mut chunk);

                let chunkptr = Arc::new(RwLock::new(chunk));
                tx.send((chunkpos, chunkptr)).unwrap();
            });

            task.detach();
            chunks_loading.insert(chunkpos);

            info!("ChunkLoad Enqueue {} / {}", chunk_sys.num_chunks(), chunkpos);
        });
    }

    
    while let Ok((chunkpos, chunkptr)) = rx_chunks_loading.try_recv() {
        chunks_loading.remove(&chunkpos);

        {
            let mut chunk = chunkptr.write().unwrap();

            chunk.entity = cmds.spawn((
                ChunkComponent::new(chunkpos),
                Transform::from_translation(chunkpos.as_vec3()),
                GlobalTransform::IDENTITY,  // really?
                RigidBody::Static,
            )).id();
        }

        chunk_sys.spawn_chunk(chunkptr);

        info!("ChunkLoad Completed {} / {}", chunk_sys.num_chunks(), chunkpos);
    }


    // Unload Chunks
    // 野蛮区块卸载检测
    let chunkpos_all =  Vec::from_iter(chunk_sys.get_chunks().keys().cloned());
    for chunkpos in chunkpos_all {
        let mut any_desire = false;

        for player in server.online_players.values_mut() {
            if in_load_distance(Chunk::as_chunkpos(player.position.as_ivec3()), chunkpos, player.chunks_load_distance) {
                any_desire = true;
            }
        }

        if !any_desire {
            let chunkptr = chunk_sys.despawn_chunk(chunkpos);
            let entity = chunkptr.unwrap().read().unwrap().entity;
            cmds.entity(entity).despawn_recursive();

            net_server.broadcast_packet(&SPacket::ChunkDel { chunkpos });
            
            for player in server.online_players.values_mut() {
                player.chunks_loaded.remove(&chunkpos);
            }

            info!("Chunk Unloaded {}", chunk_sys.num_chunks());
        }
    }


    // Send Chunk to Players
    // 野蛮的方法 把所有区块发给所有玩家
    for player in server.online_players.values_mut() {

        let mut n = 0;
        for (chunkpos, chunkptr) in chunk_sys.get_chunks() {
            if !player.chunks_loaded.contains(chunkpos) {
                if n > 3 {  // 不能一次性给玩家发送太多数据包 否则会溢出缓冲区 "send channel 2 with error: reliable channel memory usage was exausted"
                    break;
                }
                player.chunks_loaded.insert(*chunkpos);
                n+=1;

                info!("Send Chunk {}/{} {} to Player {}", player.chunks_loaded.len(), chunk_sys.num_chunks(), n, player.username);
                let data = CellData::from_chunk(&chunkptr.read().unwrap());
                net_server.send_packet(player.client_id, &SPacket::ChunkNew { chunkpos: *chunkpos, voxel: data });
            }
        }
    }


}






#[derive(Resource)]
pub struct ServerChunkSystem {

    pub chunks: HashMap<IVec3, ChunkPtr>,
}

impl ChunkSystem for ServerChunkSystem {
    fn get_chunks(&self) -> &HashMap<IVec3, ChunkPtr> {
        &self.chunks
    }
}

impl ServerChunkSystem {
    fn new() -> Self {
        Self {
            chunks: HashMap::default(),
        }
    }

    fn spawn_chunk(&mut self, chunkptr: ChunkPtr) {
        let cp = chunkptr.read().unwrap().chunkpos;
        self.chunks.insert(cp, chunkptr);
    }

    fn despawn_chunk(&mut self, chunkpos: IVec3) -> Option<ChunkPtr> {
        self.chunks.remove(&chunkpos)
    }
}