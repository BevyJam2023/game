use std::ops::Deref;

use bevy::{ecs::system::Command, prelude::*};
use bevy_xpbd_2d::prelude::{
    Collider, Collision, ExternalAngularImpulse, ExternalImpulse, LinearVelocity, Restitution,
    RigidBody,
};
use rand::Rng;

use crate::{
    cards::{self, rules::Rule},
    game_shapes::{self, ColorMaterialAssets, GameColor, GamePolygon, Shape, ShapeAssets},
    loading::TextureAssets,
    operation::Operation,
    utils::average,
    AppState,
};

pub mod config {
    use super::Vec2;

    pub const SIZE: (f32, f32) = (1000., 1000.);
    pub const CENTER: Vec2 = Vec2::new(0., 200.);
    pub const WALL_THICKNESS: f32 = 10.;
}

#[derive(Event)]
pub struct SpawnBody {
    shape: Shape,
    transform: Transform,
    velocity: Option<LinearVelocity>,
}

#[derive(Component)]
pub struct Board;

pub struct BoardPlugin;
impl Plugin for BoardPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<SpawnBody>()
            .add_systems(OnEnter(AppState::Playing), setup)
            .add_systems(
                Update,
                (spawn_bodies, spawn_on_timer, shape_collisions)
                    .run_if(in_state(AppState::Playing)),
            );
    }
}

fn setup(mut cmd: Commands, textures: Res<TextureAssets>) {
    cmd.spawn((
        SpriteBundle {
            sprite: Sprite {
                custom_size: Some(Vec2::new(config::SIZE.0, config::SIZE.1)),
                ..Default::default()
            },
            texture: textures.card_red.clone(),
            transform: Transform::from_translation(config::CENTER.extend(0.)),
            ..Default::default()
        },
        Board,
    ));

    let side_length = config::SIZE.0;

    // Wall positions (assuming the center of the square is at the origin)
    let positions = [
        Vec3::new(0.0, side_length / 2.0 + config::WALL_THICKNESS / 2.0, 0.0), // Top wall
        Vec3::new(0.0, -side_length / 2.0 - config::WALL_THICKNESS / 2.0, 0.0), // Bottom wall
        Vec3::new(side_length / 2.0 + config::WALL_THICKNESS / 2.0, 0.0, 0.0), // Right wall
        Vec3::new(-side_length / 2.0 - config::WALL_THICKNESS / 2.0, 0.0, 0.0), // Left wall
    ];

    // Wall sizes
    let sizes = [
        Vec3::new(side_length, config::WALL_THICKNESS, 1.0), // Horizontal walls
        Vec3::new(config::WALL_THICKNESS, side_length, 1.0), // Vertical walls
    ];

    for (i, &position) in positions.iter().enumerate() {
        let size = if i < 2 { sizes[0] } else { sizes[1] };
        let position = position + config::CENTER.extend(0.);

        cmd.spawn((
            RigidBody::Static,
            // Transform::from_translation(position),
            Collider::cuboid(size.x, size.y),
            Restitution::PERFECTLY_ELASTIC,
            SpriteBundle {
                sprite: Sprite {
                    color: Color::WHITE,
                    custom_size: Some(Vec2::new(size.x, size.y)),
                    ..Default::default()
                },
                transform: Transform::from_translation(position),
                ..Default::default()
            },
        ));
    }
}

fn spawn_on_timer(t: Res<Time>, mut e: EventWriter<SpawnBody>) {
    // let mut rng_thread = rand::thread_rng();
    //
    // e.send(SpawnBody {
    //     shape: game_shapes::Shape {
    //         polygon: GamePolygon::Hexagon,
    //         color: GameColor::Blue,
    //     },
    //     transform: Transform::from_xyz(rng_thread.gen_range(-300..=300) as f32, 0., 10.),
    // });
}

fn spawn_bodies(
    mut cmd: Commands,
    mut reader: EventReader<SpawnBody>,
    mesh: Res<ShapeAssets>,
    color_mat: Res<ColorMaterialAssets>,
) {
    for event in reader.read() {
        cmd.spawn((
            event.shape.get_bundle(&mesh, &color_mat),
            event.shape.polygon.create_collider(),
            event.shape.clone(),
            RigidBody::Dynamic,
            // ExternalImpulse::new(99999. * Vec2::Y).with_persistence(true),
            ExternalAngularImpulse::new(999.).with_persistence(true),
            Restitution::PERFECTLY_ELASTIC,
        ))
        .insert(event.transform.with_scale(Vec3::splat(0.5)));
    }
}

fn shape_collisions(
    rules: Query<&Rule>,
    q_shape: Query<(&Shape, &Transform, &LinearVelocity)>,
    mut collision_event_reader: EventReader<Collision>,
    mut s_event: EventWriter<SpawnBody>,
) {
    let Ok(rule_ops) = rules.get_single() else {
        return;
    };

    let mut combined: Vec<&Entity> = Vec::new();

    for Collision(contacts) in collision_event_reader.read() {
        // TODO:
        // Combinations / interactions occur based on the 'Rules'
        //
        let Ok((c_s1, transform1, lin_v1)) = q_shape.get(contacts.entity1) else {
            continue;
        };
        let Ok((c_s2, transform2, lin_v2)) = q_shape.get(contacts.entity2) else {
            continue;
        };
        if combined.contains(&&contacts.entity1) || combined.contains(&&contacts.entity2) {
            continue;
        };

        let polygons_slc = [c_s1.polygon, c_s2.polygon];

        if let Some(spawn_event) = rule_ops
            .iter()
            .filter(|op| match op {
                Operation::Add(s1, s2) => {
                    polygons_slc.contains(&s1.polygon) && polygons_slc.contains(&s2.polygon)
                },
                Operation::Sub(s1, s2) => {
                    polygons_slc.contains(&s1.polygon) && polygons_slc.contains(&s2.polygon)
                },
                _ => false,
            })
            .last()
            .map(|op| match op {
                Operation::Add(s1, s2) => SpawnBody {
                    shape: Shape {
                        polygon: s1.polygon + s2.polygon,
                        color: s1.color.fight(s2.color),
                    },
                    transform: Transform::from_translation(average(&[
                        transform1.translation,
                        transform2.translation,
                    ])),
                    velocity: Some(LinearVelocity(average(&[*lin_v1.deref(), *lin_v2.deref()]))),
                },
                Operation::Sub(s1, s2) => SpawnBody {
                    shape: Shape {
                        polygon: s1.polygon - s2.polygon,
                        color: s1.color.fight(s2.color),
                    },
                    transform: Transform::from_translation(average(&[
                        transform1.translation,
                        transform2.translation,
                    ])),
                    velocity: Some(LinearVelocity(average(&[*lin_v1.deref(), *lin_v2.deref()]))),
                },
                _ => unreachable!(),
            })
        {
            s_event.send(spawn_event);
            combined.append(&mut vec![&contacts.entity1, &contacts.entity2]);
        }
    }
}

// NOTE:
// Reference for getting image size :
// if let Some(image) = images.get(&textures.card_king) {
//     // Get the dimensions of the image
//     let dimensions = image.texture_descriptor.size;
//     println!("Image Dimensions: {:?}", dimensions);
// } else {
//     println!("Image not loaded yet");
// }

pub fn get_deck_transform(board_size: Vec2, card_height: f32) -> Transform {
    Transform::from_xyz(-board_size.x / 2., -(board_size.y + card_height) / 2., 20.)
}

pub fn get_discard_transform(board_size: Vec2, card_height: f32) -> Transform {
    Transform::from_xyz(board_size.x / 2., -(board_size.y + card_height) / 2., 20.)
}
