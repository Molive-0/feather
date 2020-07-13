//! Implements falling block entities: sand, gravel, etc.

use feather_core::blocks::{BlockId, BlockKind};
use feather_core::entitymeta::{EntityMetadata, META_INDEX_FALLING_BLOCK_SPAWN_POSITION};
use feather_core::network::packets::{Effect, SpawnObject};
use feather_core::network::Packet;
use feather_core::util::{BlockPosition, Position};
use feather_server_types::{
    BlockUpdateCause, BumpVec, EntityLandEvent, EntitySpawnEvent, Game, NetworkId, PhysicsBuilder,
    SpawnPacketCreator, Uuid, Velocity,
};
use feather_server_util::{
    degrees_to_stops, protocol_velocity, BlockNotifyBlock, BlockNotifyFallingBlock,
    BlockNotifyPosition,
};
use fecs::{component, EntityBuilder, EntityRef, IntoQuery, Read, World};

/// Marker component indicating an entity is a falling block.
#[derive(Copy, Clone, Debug)]
pub struct FallingBlock;

/// Component storing the block type for a falling block.
#[derive(Copy, Clone, Debug)]
pub struct FallingBlockType(pub BlockId);

/// System to create a falling block when a block notify
/// entity is spawned with `BlockNotifyFallingBlock`.
#[fecs::system]
pub fn spawn_falling_blocks(game: &mut Game, world: &mut World) {
    let mut actions = BumpVec::new_in(game.bump());

    actions.extend(
        <(Read<BlockNotifyBlock>, Read<BlockNotifyPosition>)>::query()
            .filter(component::<BlockNotifyFallingBlock>())
            .iter_entities(world.inner())
            .map(|(entity, (block, position))| {
                let builder = if game.block_at(position.0 - BlockPosition::new(0, 1, 0))
                    == Some(BlockId::air())
                {
                    Some(
                        create(block.0, position.0)
                            .with(position.0.position() + position!(0.0, -0.5, 0.0)),
                    )
                } else {
                    None
                };

                (entity, builder, position.0)
            }),
    );

    for (entity_to_delete, entity_builder, block_to_clear) in actions {
        world.despawn(entity_to_delete);

        if let Some(entity_builder) = entity_builder {
            let created_entity = entity_builder.build().spawn_in(world);
            game.handle(
                world,
                EntitySpawnEvent {
                    entity: created_entity,
                },
            );

            game.set_block_at(
                world,
                block_to_clear,
                BlockId::air(),
                BlockUpdateCause::Unknown,
            );
        }
    }
}

/// When a falling block lands on the ground, deletes
/// it and creates a solid block where it landed.
#[fecs::event_handler]
pub fn on_entity_land_remove_falling_block(
    event: &EntityLandEvent,
    game: &mut Game,
    world: &mut World,
) {
    if let Some(block) = world
        .try_get::<FallingBlockType>(event.entity)
        .map(|block| block.0)
    {
        let pos = event.pos.block();
        game.set_block_at(world, pos, block, BlockUpdateCause::Unknown);

        game.despawn(event.entity, world);

        if matches!(
            block.kind(),
            BlockKind::Anvil | BlockKind::ChippedAnvil | BlockKind::DamagedAnvil
        ) {
            game.broadcast_chunk_update(
                world,
                Effect {
                    effect_id: 1031, // TODO remove hardcoded magic number
                    location: pos,
                    data: 0,
                    disable_relative_volume: false,
                },
                event.pos.chunk(),
                None,
            );
        }
    }
}

/// Returns an `EntityBuilder` for a falling block of the given type.
pub fn create(ty: BlockId, spawn_pos: BlockPosition) -> EntityBuilder {
    let meta =
        EntityMetadata::entity_base().with(META_INDEX_FALLING_BLOCK_SPAWN_POSITION, spawn_pos);

    crate::base()
        .with(FallingBlock)
        .with(FallingBlockType(ty))
        .with(SpawnPacketCreator(&create_spawn_packet))
        .with(
            PhysicsBuilder::new()
                .bbox(0.98, 0.98, 0.98)
                .drag(0.98)
                .gravity(-0.04)
                .build(),
        )
        .with(meta)
}

fn create_spawn_packet(accessor: &EntityRef) -> Box<dyn Packet> {
    let data = i32::from(accessor.get::<FallingBlockType>().0.vanilla_id());
    let position = accessor.get::<Position>();
    let entity_id = accessor.get::<NetworkId>().0;

    let velocity = accessor.get::<Velocity>().0;

    let (velocity_x, velocity_y, velocity_z) = protocol_velocity(velocity);

    let packet = SpawnObject {
        entity_id,
        object_uuid: Uuid::new_v4(),
        ty: 70, // Type 70 for falling block
        x: position.x,
        y: position.y,
        z: position.z,
        pitch: degrees_to_stops(position.pitch),
        yaw: degrees_to_stops(position.yaw),
        data,
        velocity_x,
        velocity_y,
        velocity_z,
    };

    Box::new(packet)
}
