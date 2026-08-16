use bevy::color::LinearRgba;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowPlugin, WindowResolution};
use servus_sim::{
    CommandOutcome, FoundationKind, GameCommand, GridPosition, MapSize, ServiceId, ServiceKind,
    ServiceState, ServiceTier, Simulation, SolutionId,
};

const FLOOR_HEIGHT: f32 = 0.72;
const FLOOR_GAP: f32 = 0.08;
const BUILDING_WIDTH: f32 = 4.0;
const BASE_HEIGHT: f32 = 0.5;
const TICK_SECONDS: f32 = 1.0;
const CITY_TILE: f32 = 2.7;

#[derive(Resource)]
struct PrototypeSimulation {
    simulation: Simulation,
    timer: Timer,
}

#[derive(Resource, Default)]
struct SelectedFloor(Option<ServiceId>);

#[derive(Resource)]
struct PrototypeBuildTool {
    action: PrototypeBuildAction,
    feedback: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrototypeBuildAction {
    Foundation(FoundationKind),
    Service(ServiceKind),
}

#[derive(Component)]
struct Floor3d {
    service: ServiceId,
    bounds: Bounds3d,
}

#[derive(Component)]
struct Solution3d {
    solution: SolutionId,
    bounds: Bounds3d,
}

#[derive(Component)]
struct PrototypeBuildButton(PrototypeBuildAction);

#[derive(Component)]
struct PrototypeFeedback;

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
        .insert_resource(PrototypeBuildTool {
            action: PrototypeBuildAction::Foundation(FoundationKind::SmallLot),
            feedback: "Choose a lot, then click the city grid".to_owned(),
        })
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
                handle_build_palette,
                place_from_build_tool,
                sync_3d_scene,
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
    Simulation::new(
        3_000,
        0,
        MapSize::new(8, 8).expect("prototype map dimensions are valid"),
    )
}

fn setup_3d(
    mut commands: Commands,
    _prototype: Res<PrototypeSimulation>,
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
        Text::new("SERVUS 3D\n\nLeft-click  Build\nRight-click floor  Inspect\nU  Upgrade inspected floor\nDrag  Orbit camera\nMouse wheel  Zoom"),
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
    commands.spawn((
        Text::new("Choose a lot, then click the city grid"),
        TextFont::from_font_size(14.0),
        TextColor(Color::srgb(0.95, 0.82, 0.35)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(305.0),
            top: Val::Px(20.0),
            right: Val::Px(325.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        PrototypeFeedback,
    ));
    spawn_build_palette(&mut commands);
}

fn spawn_build_palette(commands: &mut Commands) {
    let actions = [
        (
            "SMALL LOT\n100c",
            PrototypeBuildAction::Foundation(FoundationKind::SmallLot),
        ),
        (
            "TOWER LOT\n250c",
            PrototypeBuildAction::Foundation(FoundationKind::TowerLot),
        ),
        (
            "MEGATOWER\n500c",
            PrototypeBuildAction::Foundation(FoundationKind::MegatowerLot),
        ),
        (
            "GW\n50c",
            PrototypeBuildAction::Service(ServiceKind::InternetGateway),
        ),
        (
            "FW\n125c",
            PrototypeBuildAction::Service(ServiceKind::Firewall),
        ),
        (
            "LB\n75c",
            PrototypeBuildAction::Service(ServiceKind::LoadBalancer),
        ),
        (
            "APP\n100c",
            PrototypeBuildAction::Service(ServiceKind::ApplicationServer),
        ),
        (
            "QUEUE\n90c",
            PrototypeBuildAction::Service(ServiceKind::MessageQueue),
        ),
        (
            "SQL\n180c",
            PrototypeBuildAction::Service(ServiceKind::RelationalDatabase),
        ),
    ];
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(300.0),
                right: Val::Px(315.0),
                bottom: Val::Px(20.0),
                padding: UiRect::all(Val::Px(10.0)),
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(6.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                ..default()
            },
            BackgroundColor(Color::srgba(0.035, 0.065, 0.1, 0.96)),
            ZIndex(20),
        ))
        .with_children(|row| {
            for (label, action) in actions {
                row.spawn((
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(7.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.08, 0.16, 0.24)),
                    PrototypeBuildButton(action),
                ))
                .with_child((
                    Text::new(label),
                    TextFont::from_font_size(11.0),
                    TextColor(Color::WHITE),
                ));
            }
        });
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

fn handle_build_palette(
    mut tool: ResMut<PrototypeBuildTool>,
    mut buttons: Query<(&Interaction, &PrototypeBuildButton, &mut BackgroundColor)>,
) {
    for (interaction, button, mut background) in &mut buttons {
        if *interaction == Interaction::Pressed {
            tool.action = button.0;
            tool.feedback = match button.0 {
                PrototypeBuildAction::Foundation(foundation) => format!(
                    "{} selected — click an empty grid area",
                    foundation_name(foundation)
                ),
                PrototypeBuildAction::Service(kind) => {
                    format!("{} selected — click a building", kind_name(kind))
                }
            };
        }
        background.0 = if tool.action == button.0 {
            Color::srgb(0.08, 0.48, 0.72)
        } else if *interaction == Interaction::Hovered {
            Color::srgb(0.15, 0.38, 0.55)
        } else {
            Color::srgb(0.08, 0.16, 0.24)
        };
    }
}

fn place_from_build_tool(
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    buttons: Query<&Interaction, With<PrototypeBuildButton>>,
    solutions: Query<&Solution3d>,
    mut prototype: ResMut<PrototypeSimulation>,
    mut tool: ResMut<PrototypeBuildTool>,
) {
    if !mouse.just_pressed(MouseButton::Left)
        || buttons
            .iter()
            .any(|interaction| *interaction != Interaction::None)
    {
        return;
    }
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let (camera, camera_transform) = *camera;
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor) else {
        return;
    };
    match tool.action {
        PrototypeBuildAction::Foundation(foundation) => {
            let Some(world) = ray_ground_intersection(ray.origin, *ray.direction) else {
                return;
            };
            let Some(position) = world_to_city_grid(prototype.simulation.map().size(), world)
            else {
                tool.feedback = "Build inside the marked city grid".to_owned();
                return;
            };
            match prototype.simulation.apply(GameCommand::BuildSolution {
                foundation,
                position,
            }) {
                Ok(CommandOutcome::SolutionBuilt { id, .. }) => {
                    tool.feedback = format!(
                        "Built {} #{} — select a service below",
                        foundation_name(foundation),
                        id.value()
                    );
                    tool.action = PrototypeBuildAction::Service(ServiceKind::InternetGateway);
                }
                Ok(_) => unreachable!("foundation command returns a foundation outcome"),
                Err(error) => tool.feedback = format!("Cannot place lot: {error}"),
            }
        }
        PrototypeBuildAction::Service(kind) => {
            let target = solutions
                .iter()
                .filter_map(|solution| {
                    ray_aabb_distance(ray.origin, *ray.direction, solution.bounds)
                        .map(|distance| (distance, solution.solution))
                })
                .min_by(|left, right| left.0.total_cmp(&right.0))
                .map(|(_, solution)| solution);
            let Some(solution) = target else {
                tool.feedback = "Click a building to install this service".to_owned();
                return;
            };
            match prototype
                .simulation
                .apply(GameCommand::InstallService { solution, kind })
            {
                Ok(CommandOutcome::ServiceInstalled { id, .. }) => {
                    tool.feedback =
                        format!("Installed {} as floor #{}", kind_name(kind), id.value());
                }
                Ok(_) => unreachable!("install command returns an installation outcome"),
                Err(error) => tool.feedback = format!("Cannot install service: {error}"),
            }
        }
    }
}

fn sync_3d_scene(
    mut commands: Commands,
    prototype: Res<PrototypeSimulation>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut solution_visuals: Query<&mut Solution3d>,
    floor_visuals: Query<&Floor3d>,
) {
    let map_size = prototype.simulation.map().size();
    for solution in prototype.simulation.solutions() {
        let center = solution_world_center(map_size, solution.position(), solution.foundation());
        let footprint = solution.foundation().footprint();
        let width = f32::from(footprint.width()) * CITY_TILE - 0.25;
        let depth = f32::from(footprint.height()) * CITY_TILE - 0.25;
        let building_height =
            BASE_HEIGHT + solution.floor_count() as f32 * (FLOOR_HEIGHT + FLOOR_GAP);
        let bounds = Bounds3d {
            min: center + Vec3::new(-width / 2.0, 0.0, -depth / 2.0),
            max: center + Vec3::new(width / 2.0, building_height, depth / 2.0),
        };
        if let Some(mut visual) = solution_visuals
            .iter_mut()
            .find(|visual| visual.solution == solution.id())
        {
            visual.bounds = bounds;
        } else {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(width, BASE_HEIGHT, depth))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.18, 0.24, 0.28),
                    metallic: 0.35,
                    perceptual_roughness: 0.5,
                    ..default()
                })),
                Transform::from_translation(center + Vec3::Y * BASE_HEIGHT / 2.0),
                Solution3d {
                    solution: solution.id(),
                    bounds,
                },
            ));
        }
        for (floor, service_id) in solution.services().iter().enumerate() {
            if floor_visuals
                .iter()
                .any(|visual| visual.service == *service_id)
            {
                continue;
            }
            let Some(service) = prototype.simulation.service(*service_id) else {
                continue;
            };
            let floor_center = center + floor_center(floor);
            let local_bounds = floor_bounds(floor, service.tier());
            let bounds = Bounds3d {
                min: local_bounds.min + center,
                max: local_bounds.max + center,
            };
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(width - 0.2, FLOOR_HEIGHT, depth - 0.2))),
                MeshMaterial3d(materials.add(floor_material(service.kind()))),
                Transform::from_translation(floor_center),
                Floor3d {
                    service: service.id(),
                    bounds,
                },
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
    if !mouse.just_pressed(MouseButton::Right) {
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
    tool: Res<PrototypeBuildTool>,
    mut status: Single<&mut Text, With<PrototypeStatus>>,
    mut feedback: Single<&mut Text, (With<PrototypeFeedback>, Without<PrototypeStatus>)>,
) {
    **feedback = Text::new(tool.feedback.clone());
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

fn ray_ground_intersection(origin: Vec3, direction: Vec3) -> Option<Vec3> {
    if direction.y.abs() < f32::EPSILON {
        return None;
    }
    let distance = -origin.y / direction.y;
    (distance >= 0.0).then_some(origin + direction * distance)
}

fn city_grid_to_world(map_size: MapSize, position: GridPosition) -> Vec3 {
    let center_x = (f32::from(map_size.width()) - 1.0) / 2.0;
    let center_z = (f32::from(map_size.height()) - 1.0) / 2.0;
    Vec3::new(
        (f32::from(position.x) - center_x) * CITY_TILE,
        0.0,
        (f32::from(position.y) - center_z) * CITY_TILE,
    )
}

fn world_to_city_grid(map_size: MapSize, world: Vec3) -> Option<GridPosition> {
    let center_x = (f32::from(map_size.width()) - 1.0) / 2.0;
    let center_z = (f32::from(map_size.height()) - 1.0) / 2.0;
    let x = (world.x / CITY_TILE + center_x).round();
    let y = (world.z / CITY_TILE + center_z).round();
    if x < 0.0 || y < 0.0 || x >= f32::from(map_size.width()) || y >= f32::from(map_size.height()) {
        return None;
    }
    Some(GridPosition::new(x as u16, y as u16))
}

fn solution_world_center(
    map_size: MapSize,
    position: GridPosition,
    foundation: FoundationKind,
) -> Vec3 {
    let origin = city_grid_to_world(map_size, position);
    let footprint = foundation.footprint();
    origin
        + Vec3::new(
            (f32::from(footprint.width()) - 1.0) * CITY_TILE / 2.0,
            0.0,
            (f32::from(footprint.height()) - 1.0) * CITY_TILE / 2.0,
        )
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

fn foundation_name(foundation: FoundationKind) -> &'static str {
    match foundation {
        FoundationKind::SmallLot => "Small Lot",
        FoundationKind::TowerLot => "Tower Lot",
        FoundationKind::MegatowerLot => "Megatower Lot",
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
    fn prototype_starts_empty_and_accepts_real_build_commands() {
        let mut simulation = prototype_simulation();
        assert!(simulation.solutions().is_empty());
        let outcome = simulation
            .apply(GameCommand::BuildSolution {
                foundation: FoundationKind::SmallLot,
                position: GridPosition::new(2, 2),
            })
            .expect("prototype lot is affordable");
        let CommandOutcome::SolutionBuilt { id, .. } = outcome else {
            panic!("foundation command must build a solution")
        };
        simulation
            .apply(GameCommand::InstallService {
                solution: id,
                kind: ServiceKind::InternetGateway,
            })
            .expect("prototype service is affordable");
        assert_eq!(simulation.solutions().len(), 1);
        assert_eq!(simulation.services().len(), 1);
    }

    #[test]
    fn city_grid_coordinates_round_trip_through_3d_space() {
        let size = MapSize::new(8, 8).expect("test map is valid");
        for y in 0..size.height() {
            for x in 0..size.width() {
                let position = GridPosition::new(x, y);
                assert_eq!(
                    world_to_city_grid(size, city_grid_to_world(size, position)),
                    Some(position)
                );
            }
        }
        assert_eq!(
            ray_ground_intersection(Vec3::new(1.0, 5.0, 2.0), -Vec3::Y),
            Some(Vec3::new(1.0, 0.0, 2.0))
        );
    }
}
