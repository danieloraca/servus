use bevy::color::LinearRgba;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowPlugin, WindowResolution};
use servus_sim::{
    CommandOutcome, FoundationKind, GameCommand, GridPosition, MapSize, ServiceId, ServiceKind,
    ServiceState, ServiceTier, Simulation,
};

const FLOOR_HEIGHT: f32 = 0.72;
const FLOOR_GAP: f32 = 0.08;
const BUILDING_WIDTH: f32 = 4.0;
const BASE_HEIGHT: f32 = 0.5;
const TICK_SECONDS: f32 = 1.0;

#[derive(Resource)]
struct PrototypeSimulation {
    simulation: Simulation,
    timer: Timer,
}

#[derive(Resource, Default)]
struct SelectedFloor(Option<ServiceId>);

#[derive(Component)]
struct Floor3d {
    service: ServiceId,
    bounds: Bounds3d,
}

#[derive(Component)]
struct PrototypeStatus;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Bounds3d {
    min: Vec3,
    max: Vec3,
}

pub fn run_3d_client() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.035, 0.055, 0.075)))
        .insert_resource(PrototypeSimulation {
            simulation: prototype_simulation(),
            timer: Timer::from_seconds(TICK_SECONDS, TimerMode::Repeating),
        })
        .insert_resource(SelectedFloor::default())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Servus — 3D City Prototype".into(),
                resolution: WindowResolution::new(1180, 760),
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup_3d)
        .add_systems(
            Update,
            (
                select_floor,
                upgrade_selected_floor,
                advance_prototype,
                update_floor_appearance,
                update_prototype_status,
                orbit_camera,
            )
                .chain(),
        )
        .run();
}

fn prototype_simulation() -> Simulation {
    let mut simulation = Simulation::new(
        3_000,
        0,
        MapSize::new(8, 8).expect("prototype map dimensions are valid"),
    );
    let outcome = simulation
        .apply(GameCommand::BuildSolution {
            foundation: FoundationKind::TowerLot,
            position: GridPosition::new(3, 3),
        })
        .expect("prototype foundation is affordable");
    let CommandOutcome::SolutionBuilt { id: solution, .. } = outcome else {
        unreachable!("foundation command returns a solution")
    };
    for kind in [
        ServiceKind::InternetGateway,
        ServiceKind::Firewall,
        ServiceKind::LoadBalancer,
        ServiceKind::ApplicationServer,
        ServiceKind::MessageQueue,
        ServiceKind::RelationalDatabase,
    ] {
        simulation
            .apply(GameCommand::InstallService { solution, kind })
            .expect("prototype floor is affordable and fits the tower");
    }
    simulation
}

fn setup_3d(
    mut commands: Commands,
    prototype: Res<PrototypeSimulation>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Tonemapping::None,
        Projection::Orthographic(OrthographicProjection {
            scale: 0.075,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(11.0, 10.0, 14.0).looking_at(Vec3::new(0.0, 2.4, 0.0), Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 12_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, -0.5, 0.0)),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(26.0, 26.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.07, 0.12, 0.14),
            perceptual_roughness: 0.94,
            ..default()
        })),
    ));
    spawn_grid(&mut commands, &mut meshes, &mut materials);

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(
            BUILDING_WIDTH + 0.5,
            BASE_HEIGHT,
            BUILDING_WIDTH + 0.5,
        ))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.24, 0.28),
            metallic: 0.35,
            perceptual_roughness: 0.5,
            ..default()
        })),
        Transform::from_xyz(0.0, BASE_HEIGHT / 2.0, 0.0),
    ));

    for (floor, service) in prototype.simulation.services().iter().enumerate() {
        let center = floor_center(floor);
        let bounds = floor_bounds(floor, service.tier());
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(BUILDING_WIDTH, FLOOR_HEIGHT, BUILDING_WIDTH))),
            MeshMaterial3d(materials.add(floor_material(service.kind()))),
            Transform::from_translation(center),
            Floor3d {
                service: service.id(),
                bounds,
            },
        ));
    }

    commands.spawn((
        Text::new("SERVUS 3D PROTOTYPE\n\nClick a floor to inspect it\nU  Upgrade selected floor\nDrag  Orbit camera\nMouse wheel  Zoom"),
        TextFont::from_font_size(16.0),
        TextColor(Color::srgb(0.85, 0.93, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(20.0),
            top: Val::Px(20.0),
            padding: UiRect::all(Val::Px(15.0)),
            width: Val::Px(260.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.035, 0.065, 0.1, 0.94)),
    ));
    commands.spawn((
        Text::new("Select a floor"),
        TextFont::from_font_size(15.0),
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(20.0),
            top: Val::Px(20.0),
            padding: UiRect::all(Val::Px(15.0)),
            width: Val::Px(285.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.035, 0.065, 0.1, 0.94)),
        PrototypeStatus,
    ));
}

fn spawn_grid(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.23, 0.25),
        emissive: LinearRgba::rgb(0.02, 0.06, 0.07),
        ..default()
    });
    for offset in -6..=6 {
        let coordinate = offset as f32 * 2.0;
        for (size, position) in [
            (
                Vec3::new(24.0, 0.025, 0.025),
                Vec3::new(0.0, 0.015, coordinate),
            ),
            (
                Vec3::new(0.025, 0.025, 24.0),
                Vec3::new(coordinate, 0.015, 0.0),
            ),
        ] {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(size))),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(position),
            ));
        }
    }
}

fn select_floor(
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    floors: Query<&Floor3d>,
    mut selected: ResMut<SelectedFloor>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let (camera, camera_transform) = *camera;
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor) else {
        return;
    };
    selected.0 = floors
        .iter()
        .filter_map(|floor| {
            ray_aabb_distance(ray.origin, *ray.direction, floor.bounds)
                .map(|distance| (distance, floor.service))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, service)| service);
}

fn upgrade_selected_floor(
    keys: Res<ButtonInput<KeyCode>>,
    mut prototype: ResMut<PrototypeSimulation>,
    selected: Res<SelectedFloor>,
) {
    if !keys.just_pressed(KeyCode::KeyU) {
        return;
    }
    if let Some(id) = selected.0 {
        let _ = prototype
            .simulation
            .apply(GameCommand::UpgradeService { id });
    }
}

fn advance_prototype(time: Res<Time>, mut prototype: ResMut<PrototypeSimulation>) {
    prototype.timer.tick(time.delta());
    if prototype.timer.just_finished() {
        prototype.simulation.advance();
    }
}

fn update_floor_appearance(
    time: Res<Time>,
    prototype: Res<PrototypeSimulation>,
    selected: Res<SelectedFloor>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut floors: Query<(&Floor3d, &MeshMaterial3d<StandardMaterial>, &mut Transform)>,
) {
    for (floor, material_handle, mut transform) in &mut floors {
        let Some(service) = prototype.simulation.service(floor.service) else {
            continue;
        };
        let Some(mut material) = materials.get_mut(&material_handle.0) else {
            continue;
        };
        let base = kind_color(service.kind());
        material.base_color = match service.state() {
            ServiceState::UnderConstruction { .. } => base.mix(&Color::BLACK, 0.58),
            ServiceState::Disrupted { .. } => Color::srgb(0.85, 0.05, 0.05),
            ServiceState::Upgrading { .. } => {
                let pulse = 0.55 + time.elapsed_secs().sin().abs() * 0.45;
                Color::srgb(0.08, 0.5 * pulse, 1.0 * pulse)
            }
            ServiceState::Operational => base,
        };
        material.emissive = if selected.0 == Some(service.id()) {
            LinearRgba::rgb(0.12, 0.5, 0.72)
        } else if matches!(service.state(), ServiceState::Upgrading { .. }) {
            LinearRgba::rgb(0.02, 0.18, 0.55)
        } else {
            LinearRgba::BLACK
        };
        let tier_scale = tier_scale(service.tier());
        transform.scale = Vec3::new(tier_scale, 1.0, tier_scale);
    }
}

fn update_prototype_status(
    prototype: Res<PrototypeSimulation>,
    selected: Res<SelectedFloor>,
    mut status: Single<&mut Text, With<PrototypeStatus>>,
) {
    let Some(service) = selected.0.and_then(|id| prototype.simulation.service(id)) else {
        **status = Text::new("FLOOR INSPECTOR\n\nClick a building floor");
        return;
    };
    let upgrade = service.next_tier().map_or_else(
        || "Maximum tier".to_owned(),
        |tier| {
            format!(
                "{} credits → {} capacity",
                service.next_upgrade_cost().expect("next tier has a cost"),
                service.kind().traffic_capacity_at(tier)
            )
        },
    );
    **status = Text::new(format!(
        "FLOOR INSPECTOR\n\n{}\nTier       {}\nState      {}\nCapacity   {}\nRun cost   {}/tick\n\nUPGRADE\n{}\n\nCredits    {}",
        kind_name(service.kind()),
        service.tier(),
        service.state(),
        service.kind().traffic_capacity_at(service.tier()),
        service.kind().operating_cost_at(service.tier()),
        upgrade,
        prototype.simulation.budget().credits(),
    ));
}

fn orbit_camera(
    mouse: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<bevy::input::mouse::MouseMotion>,
    mut scroll: MessageReader<bevy::input::mouse::MouseWheel>,
    mut camera: Single<(&mut Transform, &mut Projection), With<Camera3d>>,
) {
    let (transform, projection) = &mut *camera;
    if mouse.pressed(MouseButton::Middle) || mouse.pressed(MouseButton::Right) {
        let delta: Vec2 = motion.read().map(|event| event.delta).sum();
        if delta != Vec2::ZERO {
            let target = Vec3::new(0.0, 2.4, 0.0);
            let offset = transform.translation - target;
            let yaw = Quat::from_rotation_y(-delta.x * 0.007);
            let right = transform.rotation * Vec3::X;
            let pitch = Quat::from_axis_angle(right, -delta.y * 0.005);
            transform.translation = target + yaw * pitch * offset;
            transform.look_at(target, Vec3::Y);
        }
    }
    let zoom: f32 = scroll.read().map(|event| event.y).sum();
    if let Projection::Orthographic(orthographic) = &mut **projection {
        orthographic.scale = (orthographic.scale * 0.88_f32.powf(zoom)).clamp(0.035, 0.15);
    }
}

fn floor_center(floor: usize) -> Vec3 {
    Vec3::new(
        0.0,
        BASE_HEIGHT + FLOOR_GAP + FLOOR_HEIGHT / 2.0 + floor as f32 * (FLOOR_HEIGHT + FLOOR_GAP),
        0.0,
    )
}

fn floor_bounds(floor: usize, tier: ServiceTier) -> Bounds3d {
    let center = floor_center(floor);
    let half = Vec3::new(
        BUILDING_WIDTH * tier_scale(tier) / 2.0,
        FLOOR_HEIGHT / 2.0,
        BUILDING_WIDTH * tier_scale(tier) / 2.0,
    );
    Bounds3d {
        min: center - half,
        max: center + half,
    }
}

fn ray_aabb_distance(origin: Vec3, direction: Vec3, bounds: Bounds3d) -> Option<f32> {
    let inverse = Vec3::new(
        safe_inverse(direction.x),
        safe_inverse(direction.y),
        safe_inverse(direction.z),
    );
    let first = (bounds.min - origin) * inverse;
    let second = (bounds.max - origin) * inverse;
    let near = first.min(second).max_element();
    let far = first.max(second).min_element();
    (far >= near.max(0.0)).then_some(near.max(0.0))
}

fn safe_inverse(value: f32) -> f32 {
    if value.abs() < f32::EPSILON {
        f32::INFINITY
    } else {
        value.recip()
    }
}

fn tier_scale(tier: ServiceTier) -> f32 {
    match tier {
        ServiceTier::Starter => 1.0,
        ServiceTier::Scaled => 1.08,
        ServiceTier::Enterprise => 1.16,
    }
}

fn floor_material(kind: ServiceKind) -> StandardMaterial {
    StandardMaterial {
        base_color: kind_color(kind),
        metallic: 0.18,
        perceptual_roughness: 0.48,
        ..default()
    }
}

fn kind_color(kind: ServiceKind) -> Color {
    match kind {
        ServiceKind::InternetGateway => Color::srgb(0.1, 0.65, 0.95),
        ServiceKind::Firewall => Color::srgb(0.95, 0.35, 0.08),
        ServiceKind::LoadBalancer => Color::srgb(0.62, 0.3, 0.9),
        ServiceKind::ApplicationServer => Color::srgb(0.12, 0.72, 0.42),
        ServiceKind::RelationalDatabase => Color::srgb(0.12, 0.38, 0.9),
        ServiceKind::KeyValueStore => Color::srgb(0.85, 0.18, 0.5),
        ServiceKind::Cache => Color::srgb(0.95, 0.68, 0.08),
        ServiceKind::MessageQueue => Color::srgb(0.04, 0.7, 0.68),
        ServiceKind::PubSubTopic => Color::srgb(0.88, 0.22, 0.68),
        ServiceKind::EventBus => Color::srgb(0.95, 0.32, 0.12),
    }
}

fn kind_name(kind: ServiceKind) -> &'static str {
    match kind {
        ServiceKind::InternetGateway => "Internet Gateway",
        ServiceKind::Firewall => "Firewall",
        ServiceKind::LoadBalancer => "Load Balancer",
        ServiceKind::ApplicationServer => "Application Server",
        ServiceKind::RelationalDatabase => "Relational Database",
        ServiceKind::KeyValueStore => "Key-Value Store",
        ServiceKind::Cache => "Cache",
        ServiceKind::MessageQueue => "Message Queue",
        ServiceKind::PubSubTopic => "Pub/Sub Topic",
        ServiceKind::EventBus => "Event Bus",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floors_stack_without_overlapping_and_tiers_expand_outward() {
        let first = floor_bounds(0, ServiceTier::Starter);
        let second = floor_bounds(1, ServiceTier::Starter);
        assert!(first.max.y < second.min.y);
        let enterprise = floor_bounds(0, ServiceTier::Enterprise);
        assert!(enterprise.max.x > first.max.x);
        assert_eq!(floor_center(0).x, 0.0);
    }

    #[test]
    fn ray_selection_hits_the_nearest_floor_and_rejects_misses() {
        let bounds = Bounds3d {
            min: Vec3::splat(-1.0),
            max: Vec3::splat(1.0),
        };
        assert_eq!(
            ray_aabb_distance(Vec3::new(0.0, 0.0, 5.0), -Vec3::Z, bounds),
            Some(4.0)
        );
        assert_eq!(
            ray_aabb_distance(Vec3::new(3.0, 0.0, 5.0), -Vec3::Z, bounds),
            None
        );
    }

    #[test]
    fn prototype_uses_real_solution_and_service_data() {
        let simulation = prototype_simulation();
        assert_eq!(simulation.solutions().len(), 1);
        assert_eq!(simulation.services().len(), 6);
        assert_eq!(
            simulation.services()[0].kind(),
            ServiceKind::InternetGateway
        );
        assert_eq!(
            simulation.services()[5].kind(),
            ServiceKind::RelationalDatabase
        );
    }
}
