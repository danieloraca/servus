use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowPlugin, WindowResolution};
use servus_sim::{
    CommandError, CommandOutcome, GameCommand, GridPosition, MapSize, Service, ServiceId,
    ServiceKind, ServiceState, Simulation, TickReport,
};

use crate::create_demo_scenario;

const TILE_SIZE: f32 = 72.0;
const MAP_OFFSET_X: f32 = 120.0;
const SERVICE_SIZE: f32 = 52.0;
const TICK_SECONDS: f32 = 1.25;
const CAMERA_SPEED: f32 = 480.0;
const MIN_CAMERA_SCALE: f32 = 0.55;
const MAX_CAMERA_SCALE: f32 = 2.0;

#[derive(Resource)]
struct ClientSimulation {
    simulation: Simulation,
    last_report: Option<TickReport>,
    tick_timer: Timer,
    paused: bool,
}

#[derive(Component)]
struct ServiceVisual(ServiceId);

#[derive(Component)]
struct MetricsText;

#[derive(Resource)]
struct BuildTool {
    selected: ServiceKind,
    hovered: Option<GridPosition>,
    feedback: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct VisualStyle {
    abbreviation: &'static str,
    color: [f32; 3],
}

pub fn run_bevy_client() {
    let scenario = create_demo_scenario().expect("the built-in demo scenario must be valid");

    App::new()
        .insert_resource(ClearColor(Color::srgb(0.025, 0.04, 0.065)))
        .insert_resource(ClientSimulation {
            simulation: scenario.simulation,
            last_report: None,
            tick_timer: Timer::from_seconds(TICK_SECONDS, TimerMode::Repeating),
            paused: false,
        })
        .insert_resource(BuildTool {
            selected: ServiceKind::ApplicationServer,
            hovered: None,
            feedback: "Select with 1–3, then click a free tile".to_owned(),
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Servus — Infrastructure Strategy".into(),
                resolution: WindowResolution::new(1180, 760),
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                toggle_pause,
                select_building,
                move_camera,
                advance_simulation,
                update_service_visuals,
                update_metrics,
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            (update_hovered_tile, place_selected_service, draw_map)
                .chain()
                .after(TransformSystems::Propagate),
        )
        .run();
}

fn setup(mut commands: Commands, client: Res<ClientSimulation>) {
    commands.spawn(Camera2d);

    let map_size = client.simulation.map().size();
    for service in client.simulation.services() {
        spawn_service_visual(&mut commands, map_size, *service);
    }

    commands.spawn((
        Text::new("Loading controls…"),
        TextFont::from_font_size(18.0),
        TextColor(Color::srgb(0.84, 0.9, 0.96)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(22.0),
            top: Val::Px(22.0),
            padding: UiRect::all(Val::Px(18.0)),
            width: Val::Px(270.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.075, 0.12, 0.94)),
        MetricsText,
    ));

    commands.spawn((
        Text::new("WASD / arrows: move     Mouse wheel: zoom     Click: build"),
        TextFont::from_font_size(17.0),
        TextColor(Color::srgb(0.58, 0.68, 0.78)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(335.0),
            bottom: Val::Px(24.0),
            ..default()
        },
    ));
}

fn spawn_service_visual(commands: &mut Commands, map_size: MapSize, service: Service) {
    let style = visual_style(service.kind());
    let world_position = grid_to_world(map_size, service.position());
    commands
        .spawn((
            Sprite::from_color(
                color_for_state(style, service.state(), 0.0),
                Vec2::splat(SERVICE_SIZE),
            ),
            Transform::from_xyz(world_position.x, world_position.y, 1.0),
            ServiceVisual(service.id()),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text2d::new(style.abbreviation),
                TextFont::from_font_size(16.0),
                TextColor(Color::WHITE),
                Transform::from_xyz(0.0, 0.0, 2.0),
            ));
        });
}

fn toggle_pause(keys: Res<ButtonInput<KeyCode>>, mut client: ResMut<ClientSimulation>) {
    if keys.just_pressed(KeyCode::Space) {
        client.paused = !client.paused;
    }
}

fn select_building(keys: Res<ButtonInput<KeyCode>>, mut tool: ResMut<BuildTool>) {
    let selected = if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Numpad1) {
        Some(ServiceKind::InternetGateway)
    } else if keys.just_pressed(KeyCode::Digit2) || keys.just_pressed(KeyCode::Numpad2) {
        Some(ServiceKind::LoadBalancer)
    } else if keys.just_pressed(KeyCode::Digit3) || keys.just_pressed(KeyCode::Numpad3) {
        Some(ServiceKind::ApplicationServer)
    } else {
        None
    };

    if let Some(kind) = selected {
        tool.selected = kind;
        tool.feedback = format!("Selected {}", service_kind_name(kind));
    }
}

fn move_camera(
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
    camera: Single<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    let (mut transform, mut projection) = camera.into_inner();
    let movement = camera_movement(
        keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft),
        keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight),
        keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown),
        keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp),
    );

    let scale = match &mut *projection {
        Projection::Orthographic(orthographic) => {
            orthographic.scale = zoom_scale(orthographic.scale, scroll.delta.y);
            orthographic.scale
        }
        _ => 1.0,
    };
    transform.translation += (movement * CAMERA_SPEED * scale * time.delta_secs()).extend(0.0);
}

fn update_hovered_tile(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    client: Res<ClientSimulation>,
    mut tool: ResMut<BuildTool>,
) {
    let (camera, camera_transform) = *camera;
    tool.hovered = window
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world_2d(camera_transform, cursor).ok())
        .and_then(|world| world_to_grid(client.simulation.map().size(), world));
}

fn place_selected_service(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    mut client: ResMut<ClientSimulation>,
    mut tool: ResMut<BuildTool>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(position) = tool.hovered else {
        return;
    };

    match build_service(&mut client.simulation, tool.selected, position) {
        Ok(service) => {
            spawn_service_visual(&mut commands, client.simulation.map().size(), service);
            tool.feedback = format!(
                "Building {} at ({}, {})",
                service_kind_name(service.kind()),
                position.x,
                position.y
            );
        }
        Err(error) => {
            tool.feedback = format!("Cannot build: {error}");
        }
    }
}

fn advance_simulation(time: Res<Time>, mut client: ResMut<ClientSimulation>) {
    if client.paused {
        return;
    }

    client.tick_timer.tick(time.delta());
    if client.tick_timer.just_finished() {
        client.last_report = Some(client.simulation.advance());
    }
}

fn update_service_visuals(
    time: Res<Time>,
    client: Res<ClientSimulation>,
    mut visuals: Query<(&ServiceVisual, &mut Sprite, &mut Transform)>,
) {
    let elapsed = time.elapsed_secs();
    for (visual, mut sprite, mut transform) in &mut visuals {
        let Some(service) = client.simulation.service(visual.0) else {
            continue;
        };
        let style = visual_style(service.kind());
        sprite.color = color_for_state(style, service.state(), elapsed);
        let scale = scale_for_state(service.state(), elapsed);
        transform.scale = Vec3::splat(scale);
    }
}

fn update_metrics(
    client: Res<ClientSimulation>,
    tool: Res<BuildTool>,
    mut text: Single<&mut Text, With<MetricsText>>,
) {
    let status = if client.paused { "PAUSED" } else { "RUNNING" };
    **text = Text::new(metrics_text(&client, &tool, status));
}

fn draw_map(mut gizmos: Gizmos, client: Res<ClientSimulation>, tool: Res<BuildTool>) {
    let simulation = &client.simulation;
    let map_size = simulation.map().size();
    let grid_color = Color::srgb(0.11, 0.18, 0.25);
    let half_tile = TILE_SIZE / 2.0;

    for x in 0..=map_size.width() {
        let top_left = grid_to_world(map_size, GridPosition::new(x.min(map_size.width() - 1), 0));
        let x_position = top_left.x - half_tile
            + if x == map_size.width() {
                TILE_SIZE
            } else {
                0.0
            };
        let top = grid_to_world(map_size, GridPosition::new(0, 0)).y + half_tile;
        let bottom =
            grid_to_world(map_size, GridPosition::new(0, map_size.height() - 1)).y - half_tile;
        gizmos.line_2d(
            Vec2::new(x_position, bottom),
            Vec2::new(x_position, top),
            grid_color,
        );
    }
    for y in 0..=map_size.height() {
        let left = grid_to_world(map_size, GridPosition::new(0, 0)).x - half_tile;
        let right =
            grid_to_world(map_size, GridPosition::new(map_size.width() - 1, 0)).x + half_tile;
        let first = grid_to_world(map_size, GridPosition::new(0, y.min(map_size.height() - 1)));
        let y_position = first.y + half_tile
            - if y == map_size.height() {
                TILE_SIZE
            } else {
                0.0
            };
        gizmos.line_2d(
            Vec2::new(left, y_position),
            Vec2::new(right, y_position),
            grid_color,
        );
    }

    for link in simulation.network().links() {
        let (Some(from), Some(to)) = (simulation.service(link.from), simulation.service(link.to))
        else {
            continue;
        };
        let start = grid_to_world(map_size, from.position());
        let end = grid_to_world(map_size, to.position());
        draw_arrow(&mut gizmos, start, end, Color::srgb(0.35, 0.75, 0.95));
    }

    if let Some(position) = tool.hovered {
        let color = if can_build(simulation, tool.selected, position) {
            Color::srgb(0.35, 0.95, 0.58)
        } else {
            Color::srgb(0.95, 0.3, 0.32)
        };
        gizmos.rect_2d(
            grid_to_world(map_size, position),
            Vec2::splat(SERVICE_SIZE + 8.0),
            color,
        );
    }
}

fn draw_arrow(gizmos: &mut Gizmos, start: Vec2, end: Vec2, color: Color) {
    let direction = (end - start).normalize_or_zero();
    if direction == Vec2::ZERO {
        return;
    }
    let line_start = start + direction * (SERVICE_SIZE * 0.55);
    let tip = end - direction * (SERVICE_SIZE * 0.55);
    gizmos.line_2d(line_start, tip, color);

    let perpendicular = Vec2::new(-direction.y, direction.x);
    let arrow_length = 14.0;
    let arrow_width = 7.0;
    gizmos.line_2d(
        tip,
        tip - direction * arrow_length + perpendicular * arrow_width,
        color,
    );
    gizmos.line_2d(
        tip,
        tip - direction * arrow_length - perpendicular * arrow_width,
        color,
    );
}

fn grid_to_world(map_size: MapSize, position: GridPosition) -> Vec2 {
    let center_x = (f32::from(map_size.width()) - 1.0) / 2.0;
    let center_y = (f32::from(map_size.height()) - 1.0) / 2.0;
    Vec2::new(
        (f32::from(position.x) - center_x) * TILE_SIZE + MAP_OFFSET_X,
        (center_y - f32::from(position.y)) * TILE_SIZE,
    )
}

fn world_to_grid(map_size: MapSize, world: Vec2) -> Option<GridPosition> {
    let center_x = (f32::from(map_size.width()) - 1.0) / 2.0;
    let center_y = (f32::from(map_size.height()) - 1.0) / 2.0;
    let grid_x = ((world.x - MAP_OFFSET_X) / TILE_SIZE + center_x + 0.5).floor();
    let grid_y = (center_y - world.y / TILE_SIZE + 0.5).floor();
    if grid_x < 0.0
        || grid_y < 0.0
        || grid_x >= f32::from(map_size.width())
        || grid_y >= f32::from(map_size.height())
    {
        return None;
    }
    Some(GridPosition::new(grid_x as u16, grid_y as u16))
}

fn camera_movement(left: bool, right: bool, down: bool, up: bool) -> Vec2 {
    let direction = Vec2::new(
        f32::from(u8::from(right)) - f32::from(u8::from(left)),
        f32::from(u8::from(up)) - f32::from(u8::from(down)),
    );
    direction.normalize_or_zero()
}

fn zoom_scale(current: f32, scroll_y: f32) -> f32 {
    (current * 0.85_f32.powf(scroll_y)).clamp(MIN_CAMERA_SCALE, MAX_CAMERA_SCALE)
}

fn build_service(
    simulation: &mut Simulation,
    kind: ServiceKind,
    position: GridPosition,
) -> Result<Service, CommandError> {
    let outcome = simulation.apply(GameCommand::BuildService { kind, position })?;
    match outcome {
        CommandOutcome::ServiceBuilt { id, .. } => Ok(*simulation
            .service(id)
            .expect("a successful build command must insert its service")),
        CommandOutcome::ServicesConnected { .. } => {
            unreachable!("a build command cannot produce a connection outcome")
        }
    }
}

fn can_build(simulation: &Simulation, kind: ServiceKind, position: GridPosition) -> bool {
    let mut preview = simulation.clone();
    preview
        .apply(GameCommand::BuildService { kind, position })
        .is_ok()
}

fn visual_style(kind: ServiceKind) -> VisualStyle {
    match kind {
        ServiceKind::InternetGateway => VisualStyle {
            abbreviation: "GW",
            color: [0.15, 0.72, 0.92],
        },
        ServiceKind::LoadBalancer => VisualStyle {
            abbreviation: "LB",
            color: [0.67, 0.38, 0.92],
        },
        ServiceKind::ApplicationServer => VisualStyle {
            abbreviation: "APP",
            color: [0.2, 0.78, 0.5],
        },
    }
}

fn service_kind_name(kind: ServiceKind) -> &'static str {
    match kind {
        ServiceKind::InternetGateway => "Internet Gateway",
        ServiceKind::LoadBalancer => "Load Balancer",
        ServiceKind::ApplicationServer => "Application Server",
    }
}

fn color_for_state(style: VisualStyle, state: ServiceState, elapsed: f32) -> Color {
    let [red, green, blue] = style.color;
    match state {
        ServiceState::Operational => Color::srgb(red, green, blue),
        ServiceState::UnderConstruction { .. } => {
            let alpha = 0.42 + elapsed.sin().abs() * 0.25;
            Color::srgba(red * 0.65, green * 0.65, blue * 0.65, alpha)
        }
    }
}

fn scale_for_state(state: ServiceState, elapsed: f32) -> f32 {
    match state {
        ServiceState::Operational => 1.0,
        ServiceState::UnderConstruction { .. } => 0.94 + elapsed.sin().abs() * 0.06,
    }
}

fn metrics_text(client: &ClientSimulation, tool: &BuildTool, status: &str) -> String {
    let simulation = &client.simulation;
    let report = client.last_report.as_ref();
    let received = report.map_or(0, |report| report.received);
    let served = report.map_or(0, |report| report.served);
    let dropped = report.map_or(0, |report| report.dropped);
    format!(
        "SERVUS  {status}\n\nTick       {:>6}\nCredits    {:>6}\nDemand     {:>6}\nServed     {:>6}\nDropped    {:>6}\n\nBUILD\n1  Gateway       50\n2  Load Balancer 75\n3  App Server   100\n\nSelected: {} ({})\n{}\n\nSpace: pause / resume",
        simulation.tick().number(),
        simulation.budget().credits(),
        received,
        served,
        dropped,
        service_kind_name(tool.selected),
        tool.selected.build_cost(),
        tool.feedback,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_positions_are_centered_and_y_runs_down_the_map() {
        let map = MapSize::new(3, 3).expect("test map is valid");
        assert_eq!(
            grid_to_world(map, GridPosition::new(1, 1)),
            Vec2::new(MAP_OFFSET_X, 0.0)
        );
        assert_eq!(
            grid_to_world(map, GridPosition::new(0, 0)),
            Vec2::new(MAP_OFFSET_X - TILE_SIZE, TILE_SIZE)
        );
        assert_eq!(
            grid_to_world(map, GridPosition::new(2, 2)),
            Vec2::new(MAP_OFFSET_X + TILE_SIZE, -TILE_SIZE)
        );
    }

    #[test]
    fn every_grid_position_round_trips_through_world_space() {
        let map = MapSize::new(8, 8).expect("test map is valid");
        for y in 0..map.height() {
            for x in 0..map.width() {
                let position = GridPosition::new(x, y);
                assert_eq!(
                    world_to_grid(map, grid_to_world(map, position)),
                    Some(position)
                );
            }
        }
    }

    #[test]
    fn world_positions_outside_the_grid_are_rejected() {
        let map = MapSize::new(3, 3).expect("test map is valid");
        assert_eq!(
            world_to_grid(map, Vec2::new(MAP_OFFSET_X - 109.0, 0.0)),
            None
        );
        assert_eq!(
            world_to_grid(map, Vec2::new(MAP_OFFSET_X + 109.0, 0.0)),
            None
        );
        assert_eq!(world_to_grid(map, Vec2::new(MAP_OFFSET_X, 109.0)), None);
        assert_eq!(world_to_grid(map, Vec2::new(MAP_OFFSET_X, -109.0)), None);
    }

    #[test]
    fn camera_movement_is_normalized_and_opposites_cancel() {
        assert_eq!(camera_movement(false, false, false, false), Vec2::ZERO);
        assert_eq!(camera_movement(true, true, false, false), Vec2::ZERO);
        assert_eq!(camera_movement(false, true, false, false), Vec2::X);
        assert!((camera_movement(false, true, false, true).length() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn zoom_moves_in_the_expected_direction_and_stays_bounded() {
        assert!(zoom_scale(1.0, 1.0) < 1.0);
        assert!(zoom_scale(1.0, -1.0) > 1.0);
        assert_eq!(zoom_scale(1.0, 100.0), MIN_CAMERA_SCALE);
        assert_eq!(zoom_scale(1.0, -100.0), MAX_CAMERA_SCALE);
    }

    #[test]
    fn every_service_kind_has_a_distinct_visual_identity() {
        let gateway = visual_style(ServiceKind::InternetGateway);
        let load_balancer = visual_style(ServiceKind::LoadBalancer);
        let server = visual_style(ServiceKind::ApplicationServer);
        assert_eq!(gateway.abbreviation, "GW");
        assert_eq!(load_balancer.abbreviation, "LB");
        assert_eq!(server.abbreviation, "APP");
        assert_ne!(gateway.color, load_balancer.color);
        assert_ne!(load_balancer.color, server.color);
    }

    #[test]
    fn construction_pulses_but_operational_services_are_stable() {
        let building = ServiceState::UnderConstruction { ticks_remaining: 2 };
        assert_ne!(
            scale_for_state(building, 0.0),
            scale_for_state(building, 1.0)
        );
        assert_eq!(scale_for_state(ServiceState::Operational, 0.0), 1.0);
        assert_eq!(scale_for_state(ServiceState::Operational, 5.0), 1.0);
    }

    #[test]
    fn build_tool_places_a_service_through_the_simulation() {
        let map = MapSize::new(3, 3).expect("test map is valid");
        let mut simulation = Simulation::new(200, 0, map);
        let position = GridPosition::new(1, 2);
        assert!(can_build(
            &simulation,
            ServiceKind::ApplicationServer,
            position
        ));

        let service = build_service(&mut simulation, ServiceKind::ApplicationServer, position)
            .expect("the selected service is affordable and the tile is free");
        assert_eq!(service.kind(), ServiceKind::ApplicationServer);
        assert_eq!(service.position(), position);
        assert_eq!(simulation.budget().credits(), 100);
        assert!(!can_build(
            &simulation,
            ServiceKind::InternetGateway,
            position
        ));
    }

    #[test]
    fn failed_build_tool_placement_preserves_the_simulation() {
        let map = MapSize::new(2, 2).expect("test map is valid");
        let mut simulation = Simulation::new(40, 0, map);
        let before = simulation.clone();
        let error = build_service(
            &mut simulation,
            ServiceKind::InternetGateway,
            GridPosition::new(0, 0),
        )
        .expect_err("forty credits cannot buy a gateway");
        assert_eq!(
            error.to_string(),
            "not enough credits: 50 required, 40 available"
        );
        assert_eq!(simulation, before);
        assert!(!can_build(
            &simulation,
            ServiceKind::InternetGateway,
            GridPosition::new(0, 0)
        ));
    }

    #[test]
    fn metrics_include_the_initial_scenario_state() {
        let scenario = create_demo_scenario().expect("demo scenario is valid");
        let client = ClientSimulation {
            simulation: scenario.simulation,
            last_report: None,
            tick_timer: Timer::from_seconds(1.0, TimerMode::Repeating),
            paused: false,
        };
        let tool = BuildTool {
            selected: ServiceKind::ApplicationServer,
            hovered: None,
            feedback: "Ready".to_owned(),
        };
        let text = metrics_text(&client, &tool, "RUNNING");
        assert!(text.contains("Tick            0"));
        assert!(text.contains("Credits        45"));
        assert!(text.contains("Demand          0"));
        assert!(text.contains("Selected: Application Server (100)"));
        assert!(text.contains("Ready"));
    }
}
