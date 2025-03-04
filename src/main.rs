use std::f32::consts::TAU;

use bevy::{
    gltf::{Gltf, GltfMesh, GltfNode},
    prelude::*,
    render::camera::Exposure,
    window::CursorGrabMode,
};
use bevy_flycam::prelude::*;
use bevy_fps_controller::controller::*;
use bevy_rapier3d::prelude::*;

const SPAWN_POINT: Vec3 = Vec3::new(2.0, 1.625, 0.0);

fn main() {
    App::new()
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 10000.0,
        })
        .insert_resource(ClearColor(Color::linear_rgb(0.83, 0.96, 0.96)))
        .add_plugins(DefaultPlugins)
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        // .add_plugins(RapierDebugRenderPlugin::default())
        .add_plugins(NoCameraPlayerPlugin)
        .add_plugins(FpsControllerPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, toggle_camera)
        .add_systems(Update, (manage_cursor, scene_colliders, respawn))
        .run();
}

/// 第一人称相机，受重力影响
#[derive(Component)]
struct MainCamera;

/// 场景初始化
fn setup(mut commands: Commands, mut window: Query<&mut Window>, assets: Res<AssetServer>) {
    let mut window = window.single_mut();
    window.title = String::from("Minimal FPS Controller Example");

    // 平行光
    commands.spawn((
        DirectionalLight {
            illuminance: light_consts::lux::FULL_DAYLIGHT,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 7.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // 第一相机
    let height = 3.0;
    let logical_entity = commands
        .spawn((
            Collider::cylinder(height / 2.0, 0.5),
            // A capsule can be used but is NOT recommended
            // If you use it, you have to make sure each segment point is
            // equidistant from the translation of the player transform
            // Collider::capsule_y(height / 2.0, 0.5),
            Friction {
                coefficient: 0.0,
                combine_rule: CoefficientCombineRule::Min,
            },
            Restitution {
                coefficient: 0.0,
                combine_rule: CoefficientCombineRule::Min,
            },
            ActiveEvents::COLLISION_EVENTS,
            Velocity::zero(),
            RigidBody::Dynamic,
            Sleeping::disabled(),
            LockedAxes::ROTATION_LOCKED,
            AdditionalMassProperties::Mass(1.0),
            GravityScale(0.0),
            Ccd { enabled: true }, // Prevent clipping when going fast
            Transform::from_translation(SPAWN_POINT),
            LogicalPlayer,
            FpsControllerInput {
                pitch: -TAU / 12.0,
                yaw: TAU * 5.0 / 8.0,
                ..default()
            },
            FpsController {
                air_acceleration: 80.0,
                ..default()
            },
        ))
        .insert(CameraConfig {
            height_offset: -0.5,
        })
        .id();
    commands.spawn((
        Camera3d::default(),
        Camera {
            is_active: false,
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: TAU / 5.0,
            ..default()
        }),
        Exposure::SUNLIGHT,
        RenderPlayer { logical_entity },
        MainCamera,
    ));

    // 第二相机
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: TAU / 5.0,
            ..default()
        }),
        FlyCam,
    ));

    commands.insert_resource(MainScene {
        handle: assets.load("models/city/source/model.glb"),
        is_loaded: false,
    });
}

fn toggle_camera(mut cameras: Query<&mut Camera>, keys: Res<ButtonInput<KeyCode>>) {
    if keys.just_pressed(KeyCode::KeyC) {
        for mut camera in &mut cameras {
            camera.is_active = !camera.is_active; // 切换激活状态
        }
    }
}

/// 重生
fn respawn(mut query: Query<(&mut Transform, &mut Velocity)>) {
    for (mut transform, mut velocity) in &mut query {
        if transform.translation.y > -50.0 {
            continue;
        }

        velocity.linvel = Vec3::ZERO;
        transform.translation = SPAWN_POINT;
    }
}

#[derive(Resource)]
struct MainScene {
    handle: Handle<Gltf>,
    is_loaded: bool,
}

/// 场景碰撞体
fn scene_colliders(
    mut commands: Commands,
    mut main_scene: ResMut<MainScene>,
    gltf_assets: Res<Assets<Gltf>>,
    gltf_mesh_assets: Res<Assets<GltfMesh>>,
    gltf_node_assets: Res<Assets<GltfNode>>,
    mesh_assets: Res<Assets<Mesh>>,
) {
    if main_scene.is_loaded {
        return;
    }

    let gltf = gltf_assets.get(&main_scene.handle);
    let Some(gltf) = gltf else {
        return;
    };
    let Some(scene) = gltf.scenes.first() else {
        error!("No scenes found in gltf");
        return;
    };
    commands.spawn(SceneRoot(scene.clone()));
    for node in &gltf.nodes {
        let Some(node) = gltf_node_assets.get(node) else {
            continue;
        };
        if !node.name.ends_with("_collision") {
            continue;
        }
        let Some(gltf_mesh) = node.mesh.clone() else {
            continue;
        };
        let Some(gltf_mesh) = gltf_mesh_assets.get(&gltf_mesh) else {
            continue;
        };
        for mesh_primitive in &gltf_mesh.primitives {
            let Some(mesh) = mesh_assets.get(&mesh_primitive.mesh) else {
                continue;
            };
            let collider = Collider::from_bevy_mesh(
                mesh,
                &ComputedColliderShape::TriMesh(TriMeshFlags::all()),
            );
            let Some(collider) = collider else {
                error!("Failed to create collider from mesh");
                continue;
            };
            commands.spawn((collider, RigidBody::Fixed, node.transform));
        }
    }
    main_scene.is_loaded = true;
}

fn manage_cursor(
    btn: Res<ButtonInput<MouseButton>>,
    key: Res<ButtonInput<KeyCode>>,
    mut window_query: Query<&mut Window>,
    mut controller_query: Query<&mut FpsController>,
) {
    for mut window in &mut window_query {
        if btn.just_pressed(MouseButton::Left) {
            window.cursor_options.grab_mode = CursorGrabMode::Locked;
            window.cursor_options.visible = false;
            for mut controller in &mut controller_query {
                controller.enable_input = true;
            }
        }
        if key.just_pressed(KeyCode::Escape) {
            window.cursor_options.grab_mode = CursorGrabMode::None;
            window.cursor_options.visible = true;
            for mut controller in &mut controller_query {
                controller.enable_input = false;
            }
        }
    }
}
