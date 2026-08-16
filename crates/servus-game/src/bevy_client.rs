use std::time::Duration;

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowPlugin, WindowResolution};
use servus_sim::{
    CYBER_ATTACK_INTERVAL, CommandError, CommandOutcome, GameCommand, GridPosition, MapSize,
    NETWORK_LINK_COST, Service, ServiceId, ServiceKind, ServiceState, ServiceTier, Simulation,
    TickReport,
};

#[cfg(test)]
use crate::create_demo_scenario;
use crate::create_new_game;

const TILE_SIZE: f32 = 72.0;
const MAP_OFFSET_X: f32 = 120.0;
const SERVICE_SIZE: f32 = 52.0;
const TICK_SECONDS: f32 = 1.25;
const CAMERA_SPEED: f32 = 480.0;
const MIN_CAMERA_SCALE: f32 = 0.55;
const MAX_CAMERA_SCALE: f32 = 2.0;
const DEMAND_STEP: u64 = 50;
const MAX_DEMAND: u64 = 1_000;
const SERVED_OBJECTIVE: u64 = 500;
const NOTIFICATION_SECONDS: f32 = 3.0;
const OBJECTIVE_COUNT: usize = 9;

#[derive(Resource)]
struct ClientSimulation {
    simulation: Simulation,
    last_report: Option<TickReport>,
    tick_timer: Timer,
    paused: bool,
    total_served: u64,
    blocked_attacks: u64,
    successful_failovers: u64,
    outage_losses: u64,
    capital_invested: u64,
    total_revenue: u64,
    operating_costs: u64,
    operating_cost_shortfall: u64,
    operating_profit: i128,
}

#[derive(Component)]
struct ServiceVisual(ServiceId);

#[derive(Component)]
struct MetricsText;

#[derive(Component)]
struct NotificationText;

#[derive(Component)]
struct EconomicsText;

#[derive(Resource)]
struct BuildTool {
    selected: ServiceKind,
    hovered: Option<GridPosition>,
    network_mode: Option<NetworkMode>,
    connection_from: Option<ServiceId>,
    inspected: Option<ServiceId>,
    feedback: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NetworkMode {
    Connect,
    Disconnect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConnectionAction {
    SourceSelected(ServiceId),
    Connected { from: ServiceId, to: ServiceId },
    Disconnected { from: ServiceId, to: ServiceId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UpgradeAction {
    from: ServiceTier,
    to: ServiceTier,
    cost: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ConnectionClickError {
    EmptyTile(GridPosition),
    Command(CommandError),
}

impl std::fmt::Display for ConnectionClickError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyTile(position) => write!(
                formatter,
                "tile ({}, {}) has no service",
                position.x, position.y
            ),
            Self::Command(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ConnectionClickError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::EmptyTile(_) => None,
            Self::Command(error) => Some(error),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct VisualStyle {
    abbreviation: &'static str,
    color: [f32; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObjectiveStatus {
    label: &'static str,
    complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ProgressEvent {
    None,
    Completed(Vec<&'static str>),
    Victory,
}

#[derive(Resource)]
struct GameProgress {
    completed: [bool; OBJECTIVE_COUNT],
    won: bool,
    last_announced_attack_tick: Option<u64>,
    notification: Option<String>,
    notification_timer: Timer,
}

impl GameProgress {
    fn new() -> Self {
        Self {
            completed: [false; OBJECTIVE_COUNT],
            won: false,
            last_announced_attack_tick: None,
            notification: None,
            notification_timer: Timer::from_seconds(NOTIFICATION_SECONDS, TimerMode::Once),
        }
    }
}

pub fn run_bevy_client() {
    let simulation = create_new_game().expect("the new-game map dimensions must be valid");

    App::new()
        .insert_resource(ClearColor(Color::srgb(0.025, 0.04, 0.065)))
        .insert_resource(ClientSimulation {
            simulation,
            last_report: None,
            tick_timer: Timer::from_seconds(TICK_SECONDS, TimerMode::Repeating),
            paused: false,
            total_served: 0,
            blocked_attacks: 0,
            successful_failovers: 0,
            outage_losses: 0,
            capital_invested: 0,
            total_revenue: 0,
            operating_costs: 0,
            operating_cost_shortfall: 0,
            operating_profit: 0,
        })
        .insert_resource(BuildTool {
            selected: ServiceKind::ApplicationServer,
            hovered: None,
            network_mode: None,
            connection_from: None,
            inspected: None,
            feedback: "Select with 1–4, then click a free tile".to_owned(),
        })
        .insert_resource(GameProgress::new())
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
                restart_game,
                toggle_pause,
                select_building,
                toggle_network_mode,
                adjust_demand,
                upgrade_inspected_service,
                move_camera,
                advance_simulation,
                update_service_visuals,
                update_metrics,
                update_economics,
                update_notification,
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            (
                update_hovered_tile,
                inspect_service,
                handle_map_click,
                update_objective_progress,
                draw_map,
            )
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
        TextFont::from_font_size(12.0),
        TextColor(Color::srgb(0.84, 0.9, 0.96)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(22.0),
            top: Val::Px(22.0),
            padding: UiRect::all(Val::Px(10.0)),
            width: Val::Px(330.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.075, 0.12, 0.94)),
        MetricsText,
    ));

    commands.spawn((
        Text::new("WASD: move   1–4: build   C: connect   X: disconnect   U: upgrade   R: restart"),
        TextFont::from_font_size(16.0),
        TextColor(Color::srgb(0.58, 0.68, 0.78)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(360.0),
            bottom: Val::Px(24.0),
            ..default()
        },
    ));

    commands.spawn((
        Text::new(""),
        TextFont::from_font_size(24.0),
        TextColor(Color::srgb(1.0, 0.88, 0.35)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(24.0),
            left: Val::Percent(31.0),
            right: Val::Percent(5.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        NotificationText,
    ));

    commands.spawn((
        Text::new("Loading economy…"),
        TextFont::from_font_size(16.0),
        TextColor(Color::srgb(0.82, 0.91, 0.8)),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(22.0),
            top: Val::Px(72.0),
            padding: UiRect::all(Val::Px(14.0)),
            width: Val::Px(245.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.075, 0.12, 0.94)),
        EconomicsText,
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

fn restart_game(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    service_visuals: Query<Entity, With<ServiceVisual>>,
    mut client: ResMut<ClientSimulation>,
    mut tool: ResMut<BuildTool>,
    mut progress: ResMut<GameProgress>,
    camera: Single<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    if !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    for entity in &service_visuals {
        commands.entity(entity).despawn();
    }
    reset_scenario(&mut client, &mut tool, &mut progress);

    let (mut transform, mut projection) = camera.into_inner();
    transform.translation = Vec3::ZERO;
    if let Projection::Orthographic(orthographic) = &mut *projection {
        orthographic.scale = 1.0;
    }
}

fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    progress: Res<GameProgress>,
    mut client: ResMut<ClientSimulation>,
) {
    if keys.just_pressed(KeyCode::Space) && !progress.won {
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
    } else if keys.just_pressed(KeyCode::Digit4) || keys.just_pressed(KeyCode::Numpad4) {
        Some(ServiceKind::Firewall)
    } else {
        None
    };

    if let Some(kind) = selected {
        tool.selected = kind;
        tool.network_mode = None;
        tool.connection_from = None;
        tool.feedback = format!("Selected {}", service_kind_name(kind));
    }
}

fn toggle_network_mode(keys: Res<ButtonInput<KeyCode>>, mut tool: ResMut<BuildTool>) {
    let requested_mode = if keys.just_pressed(KeyCode::KeyC) {
        Some(NetworkMode::Connect)
    } else if keys.just_pressed(KeyCode::KeyX) {
        Some(NetworkMode::Disconnect)
    } else {
        None
    };
    if let Some(requested_mode) = requested_mode {
        tool.network_mode = (tool.network_mode != Some(requested_mode)).then_some(requested_mode);
        tool.connection_from = None;
        tool.feedback = match tool.network_mode {
            Some(NetworkMode::Connect) => "Connection mode: click the source service".to_owned(),
            Some(NetworkMode::Disconnect) => "Disconnect mode: click the source service".to_owned(),
            None => format!("Selected {}", service_kind_name(tool.selected)),
        };
    } else if keys.just_pressed(KeyCode::Escape) && tool.network_mode.is_some() {
        tool.network_mode = None;
        tool.connection_from = None;
        tool.feedback = "Network edit cancelled".to_owned();
    }
}

fn adjust_demand(
    keys: Res<ButtonInput<KeyCode>>,
    mut client: ResMut<ClientSimulation>,
    mut tool: ResMut<BuildTool>,
) {
    let increase = if keys.just_pressed(KeyCode::Equal) {
        Some(true)
    } else if keys.just_pressed(KeyCode::Minus) {
        Some(false)
    } else {
        None
    };
    let Some(increase) = increase else {
        return;
    };

    let current = client.simulation.traffic().requests_per_tick();
    let demand = adjusted_demand(current, increase);
    client.simulation.set_requests_per_tick(demand);
    tool.feedback = format!("Incoming demand set to {demand} requests/tick");
}

fn upgrade_inspected_service(
    keys: Res<ButtonInput<KeyCode>>,
    mut client: ResMut<ClientSimulation>,
    mut tool: ResMut<BuildTool>,
) {
    if !keys.just_pressed(KeyCode::KeyU) {
        return;
    }
    let Some(id) = tool.inspected else {
        tool.feedback = "Right-click a service before upgrading it".to_owned();
        return;
    };

    match try_upgrade_service(&mut client.simulation, id) {
        Ok(upgrade) => {
            client.capital_invested = client.capital_invested.saturating_add(upgrade.cost);
            tool.network_mode = None;
            tool.connection_from = None;
            tool.feedback = format!(
                "Upgrading {} from {} to {} ({} credits)",
                service_description(&client.simulation, id),
                upgrade.from,
                upgrade.to,
                upgrade.cost
            );
        }
        Err(error) => tool.feedback = format!("Cannot upgrade: {error}"),
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

fn inspect_service(
    mouse: Res<ButtonInput<MouseButton>>,
    client: Res<ClientSimulation>,
    mut tool: ResMut<BuildTool>,
) {
    if mouse.just_pressed(MouseButton::Right) {
        tool.inspected = tool
            .hovered
            .and_then(|position| client.simulation.map().service_at(position));
    }
}

fn handle_map_click(
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

    if let Some(mode) = tool.network_mode {
        match try_connection_click(
            &mut client.simulation,
            &mut tool.connection_from,
            position,
            mode,
        ) {
            Ok(ConnectionAction::SourceSelected(id)) => {
                tool.feedback = format!(
                    "Source {} selected; click a destination",
                    service_description(&client.simulation, id)
                );
            }
            Ok(ConnectionAction::Connected { from, to }) => {
                client.capital_invested = client.capital_invested.saturating_add(NETWORK_LINK_COST);
                tool.feedback = format!(
                    "Connected {} → {}",
                    service_description(&client.simulation, from),
                    service_description(&client.simulation, to)
                );
            }
            Ok(ConnectionAction::Disconnected { from, to }) => {
                tool.feedback = format!(
                    "Disconnected {} → {}",
                    service_description(&client.simulation, from),
                    service_description(&client.simulation, to)
                );
            }
            Err(error) => {
                let action = match mode {
                    NetworkMode::Connect => "connect",
                    NetworkMode::Disconnect => "disconnect",
                };
                tool.feedback = format!("Cannot {action}: {error}");
            }
        }
        return;
    }

    match build_service(&mut client.simulation, tool.selected, position) {
        Ok(service) => {
            client.capital_invested = client
                .capital_invested
                .saturating_add(service.kind().build_cost());
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
        let report = client.simulation.advance();
        client.total_served = client.total_served.saturating_add(report.served);
        if report.cyberattack.is_some_and(|attack| attack.blocked) {
            client.blocked_attacks = client.blocked_attacks.saturating_add(1);
        }
        if report.cyberattack.is_some() && report.failover_active {
            client.successful_failovers = client.successful_failovers.saturating_add(1);
        }
        client.outage_losses = client.outage_losses.saturating_add(report.outage_penalty);
        client.total_revenue = client.total_revenue.saturating_add(report.revenue);
        client.operating_costs = client.operating_costs.saturating_add(report.operating_cost);
        client.operating_cost_shortfall = client
            .operating_cost_shortfall
            .saturating_add(report.operating_cost_shortfall);
        client.operating_profit = client.operating_profit.saturating_add(report.net_income);
        client.last_report = Some(report);
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
    progress: Res<GameProgress>,
    mut text: Single<&mut Text, With<MetricsText>>,
) {
    let insolvent = client
        .last_report
        .as_ref()
        .is_some_and(|report| report.operating_cost_shortfall > 0);
    let status = if progress.won {
        "VICTORY"
    } else if insolvent {
        "INSOLVENT"
    } else if client.paused {
        "PAUSED"
    } else {
        "RUNNING"
    };
    **text = Text::new(metrics_text(&client, &tool, status));
}

fn update_economics(
    client: Res<ClientSimulation>,
    mut text: Single<&mut Text, With<EconomicsText>>,
) {
    **text = Text::new(economics_text(&client));
}

fn draw_map(
    mut gizmos: Gizmos,
    time: Res<Time>,
    client: Res<ClientSimulation>,
    tool: Res<BuildTool>,
) {
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

    for service in simulation.services() {
        let center = grid_to_world(map_size, service.position());
        for ring in 0..tier_ring_count(service.tier()) {
            gizmos.circle_2d(
                center,
                SERVICE_SIZE * 0.58 + ring as f32 * 5.0,
                Color::srgb(0.35, 0.92, 1.0),
            );
        }
    }

    for link in simulation.network().links() {
        let (Some(from), Some(to)) = (simulation.service(link.from), simulation.service(link.to))
        else {
            continue;
        };
        let start = grid_to_world(map_size, from.position());
        let end = grid_to_world(map_size, to.position());
        let selected_for_removal = tool.network_mode == Some(NetworkMode::Disconnect)
            && tool.connection_from == Some(link.from);
        let color = if selected_for_removal {
            Color::srgb(1.0, 0.28, 0.22)
        } else {
            Color::srgb(0.35, 0.75, 0.95)
        };
        draw_arrow(&mut gizmos, start, end, color);
    }

    if let Some(report) = &client.last_report {
        for traffic in &report.link_traffic {
            let (Some(from), Some(to)) = (
                simulation.service(traffic.from),
                simulation.service(traffic.to),
            ) else {
                continue;
            };
            let start = grid_to_world(map_size, from.position());
            let end = grid_to_world(map_size, to.position());
            for marker in traffic_markers(start, end, traffic.requests, time.elapsed_secs()) {
                gizmos.circle_2d(marker, 4.5, Color::srgb(1.0, 0.84, 0.28));
            }
        }
    }

    if let Some(id) = tool.inspected
        && let Some(service) = simulation.service(id)
    {
        gizmos.rect_2d(
            grid_to_world(map_size, service.position()),
            Vec2::splat(SERVICE_SIZE + 16.0),
            Color::srgb(0.3, 0.95, 1.0),
        );
    }

    if let Some(mode) = tool.network_mode {
        if let Some(from_id) = tool.connection_from
            && let Some(from) = simulation.service(from_id)
        {
            let start = grid_to_world(map_size, from.position());
            gizmos.rect_2d(
                start,
                Vec2::splat(SERVICE_SIZE + 10.0),
                Color::srgb(1.0, 0.72, 0.2),
            );
            if let Some(position) = tool.hovered {
                let end = grid_to_world(map_size, position);
                let valid = simulation
                    .map()
                    .service_at(position)
                    .is_some_and(|to| match mode {
                        NetworkMode::Connect => can_connect(simulation, from_id, to),
                        NetworkMode::Disconnect => can_disconnect(simulation, from_id, to),
                    });
                let color = simulation.map().service_at(position).map_or(
                    Color::srgb(0.95, 0.3, 0.32),
                    |_| match (mode, valid) {
                        (NetworkMode::Connect, true) => Color::srgb(0.35, 0.95, 0.58),
                        (NetworkMode::Disconnect, true) => Color::srgb(1.0, 0.55, 0.18),
                        _ => Color::srgb(0.95, 0.3, 0.32),
                    },
                );
                draw_arrow(&mut gizmos, start, end, color);
            }
        } else if let Some(position) = tool.hovered {
            let color = if simulation.map().service_at(position).is_some() {
                Color::srgb(1.0, 0.72, 0.2)
            } else {
                Color::srgb(0.95, 0.3, 0.32)
            };
            gizmos.rect_2d(
                grid_to_world(map_size, position),
                Vec2::splat(SERVICE_SIZE + 8.0),
                color,
            );
        }
    } else if let Some(position) = tool.hovered {
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

fn traffic_markers(start: Vec2, end: Vec2, requests: u64, elapsed: f32) -> Vec<Vec2> {
    if requests == 0 {
        return Vec::new();
    }
    let direction = (end - start).normalize_or_zero();
    if direction == Vec2::ZERO {
        return Vec::new();
    }
    let path_start = start + direction * (SERVICE_SIZE * 0.62);
    let path_end = end - direction * (SERVICE_SIZE * 0.62);
    let count = usize::try_from(requests.div_ceil(50).clamp(1, 4))
        .expect("the marker count is clamped to four");
    (0..count)
        .map(|index| {
            let offset = index as f32 / count as f32;
            let progress = (elapsed * 0.55 + offset).rem_euclid(1.0);
            path_start.lerp(path_end, progress)
        })
        .collect()
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

fn adjusted_demand(current: u64, increase: bool) -> u64 {
    if increase {
        current.saturating_add(DEMAND_STEP).min(MAX_DEMAND)
    } else {
        current.saturating_sub(DEMAND_STEP)
    }
}

fn ticks_until_attack(tick: u64) -> u64 {
    CYBER_ATTACK_INTERVAL - tick % CYBER_ATTACK_INTERVAL
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
        CommandOutcome::ServicesDisconnected { .. } => {
            unreachable!("a build command cannot produce a disconnection outcome")
        }
        CommandOutcome::ServiceUpgradeStarted { .. } => {
            unreachable!("a build command cannot produce an upgrade outcome")
        }
    }
}

fn can_build(simulation: &Simulation, kind: ServiceKind, position: GridPosition) -> bool {
    let mut preview = simulation.clone();
    preview
        .apply(GameCommand::BuildService { kind, position })
        .is_ok()
}

fn try_connection_click(
    simulation: &mut Simulation,
    connection_from: &mut Option<ServiceId>,
    position: GridPosition,
    mode: NetworkMode,
) -> Result<ConnectionAction, ConnectionClickError> {
    let clicked = simulation
        .map()
        .service_at(position)
        .ok_or(ConnectionClickError::EmptyTile(position))?;
    let Some(from) = *connection_from else {
        *connection_from = Some(clicked);
        return Ok(ConnectionAction::SourceSelected(clicked));
    };

    let command = match mode {
        NetworkMode::Connect => GameCommand::ConnectServices { from, to: clicked },
        NetworkMode::Disconnect => GameCommand::DisconnectServices { from, to: clicked },
    };
    let outcome = simulation
        .apply(command)
        .map_err(ConnectionClickError::Command)?;
    match outcome {
        CommandOutcome::ServicesConnected { from, to } => {
            *connection_from = None;
            Ok(ConnectionAction::Connected { from, to })
        }
        CommandOutcome::ServiceBuilt { .. } => {
            unreachable!("a network command cannot produce a build outcome")
        }
        CommandOutcome::ServicesDisconnected { from, to } => {
            *connection_from = None;
            Ok(ConnectionAction::Disconnected { from, to })
        }
        CommandOutcome::ServiceUpgradeStarted { .. } => {
            unreachable!("a network command cannot produce an upgrade outcome")
        }
    }
}

fn can_connect(simulation: &Simulation, from: ServiceId, to: ServiceId) -> bool {
    let mut preview = simulation.clone();
    preview
        .apply(GameCommand::ConnectServices { from, to })
        .is_ok()
}

fn can_disconnect(simulation: &Simulation, from: ServiceId, to: ServiceId) -> bool {
    let mut preview = simulation.clone();
    preview
        .apply(GameCommand::DisconnectServices { from, to })
        .is_ok()
}

fn try_upgrade_service(
    simulation: &mut Simulation,
    id: ServiceId,
) -> Result<UpgradeAction, CommandError> {
    let outcome = simulation.apply(GameCommand::UpgradeService { id })?;
    match outcome {
        CommandOutcome::ServiceUpgradeStarted { from, to, .. } => {
            let kind = simulation
                .service(id)
                .expect("a successful upgrade keeps the service")
                .kind();
            Ok(UpgradeAction {
                from,
                to,
                cost: kind
                    .upgrade_cost(to)
                    .expect("a successful upgrade target has a cost"),
            })
        }
        CommandOutcome::ServiceBuilt { .. }
        | CommandOutcome::ServicesConnected { .. }
        | CommandOutcome::ServicesDisconnected { .. } => {
            unreachable!("an upgrade command must produce an upgrade outcome")
        }
    }
}

fn service_description(simulation: &Simulation, id: ServiceId) -> String {
    simulation.service(id).map_or_else(
        || format!("Service #{}", id.value()),
        |service| {
            format!(
                "{} #{}",
                service_kind_name(service.kind()),
                service.id().value()
            )
        },
    )
}

fn inspection_text(client: &ClientSimulation, id: ServiceId) -> Option<String> {
    let simulation = &client.simulation;
    let service = simulation.service(id)?;
    let position = service.position();
    let incoming_links = simulation
        .network()
        .links()
        .iter()
        .filter(|link| link.to == id)
        .count();
    let outgoing_links = simulation
        .network()
        .links()
        .iter()
        .filter(|link| link.from == id)
        .count();
    let (incoming_traffic, outgoing_traffic) =
        client.last_report.as_ref().map_or((0, 0), |report| {
            report
                .link_traffic
                .iter()
                .fold((0_u64, 0_u64), |totals, traffic| {
                    (
                        totals.0 + u64::from(traffic.to == id) * traffic.requests,
                        totals.1 + u64::from(traffic.from == id) * traffic.requests,
                    )
                })
        });
    let capacity = service.kind().traffic_capacity_at(service.tier());
    let upgrade = match service.state() {
        ServiceState::Upgrading { target, .. } => format!("In progress: {target}"),
        _ => service.next_tier().map_or_else(
            || "Maximum tier".to_owned(),
            |target| {
                format!(
                    "{}: {}c / {} ticks / cap {}",
                    target,
                    service.next_upgrade_cost().expect("a next tier has a cost"),
                    service
                        .kind()
                        .upgrade_ticks(target)
                        .expect("a next tier has a duration"),
                    service.kind().traffic_capacity_at(target),
                )
            },
        ),
    };
    Some(format!(
        "INSPECT\n{} #{}\nTier          {}\nTile          ({}, {})\nState         {}\nCapacity      {} req/tick\nRun cost      {}/tick\nUpgrade       {}\nLinks in/out  {}/{}\nFlow in/out   {}/{}",
        service_kind_name(service.kind()),
        id.value(),
        service.tier(),
        position.x,
        position.y,
        service.state(),
        capacity,
        service.kind().operating_cost_at(service.tier()),
        upgrade,
        incoming_links,
        outgoing_links,
        incoming_traffic,
        outgoing_traffic,
    ))
}

fn objective_statuses(client: &ClientSimulation) -> [ObjectiveStatus; OBJECTIVE_COUNT] {
    let has_kind = |kind| {
        client
            .simulation
            .services()
            .iter()
            .any(|service| service.kind() == kind)
    };
    [
        ObjectiveStatus {
            label: "Build an Internet Gateway",
            complete: has_kind(ServiceKind::InternetGateway),
        },
        ObjectiveStatus {
            label: "Build a Firewall",
            complete: has_kind(ServiceKind::Firewall),
        },
        ObjectiveStatus {
            label: "Build a Load Balancer",
            complete: has_kind(ServiceKind::LoadBalancer),
        },
        ObjectiveStatus {
            label: "Build an Application Server",
            complete: has_kind(ServiceKind::ApplicationServer),
        },
        ObjectiveStatus {
            label: "Route a live request",
            complete: client.total_served > 0,
        },
        ObjectiveStatus {
            label: "Block a cyberattack",
            complete: client.blocked_attacks > 0,
        },
        ObjectiveStatus {
            label: "Serve 500 total requests",
            complete: client.total_served >= SERVED_OBJECTIVE,
        },
        ObjectiveStatus {
            label: "Reach positive infrastructure ROI",
            complete: client.capital_invested > 0
                && client.operating_profit >= i128::from(client.capital_invested),
        },
        ObjectiveStatus {
            label: "Upgrade infrastructure to Scaled",
            complete: client
                .simulation
                .services()
                .iter()
                .any(|service| service.tier() != ServiceTier::Starter),
        },
    ]
}

fn objectives_text(client: &ClientSimulation) -> String {
    let lines = objective_statuses(client).map(|objective| {
        let marker = if objective.complete { "[x]" } else { "[ ]" };
        format!("{marker} {}", objective.label)
    });
    format!("OBJECTIVES\n{}", lines.join("\n"))
}

fn apply_objective_progress(
    progress: &mut GameProgress,
    statuses: [ObjectiveStatus; OBJECTIVE_COUNT],
) -> ProgressEvent {
    let newly_completed = statuses
        .iter()
        .enumerate()
        .filter_map(|(index, objective)| {
            (objective.complete && !progress.completed[index]).then_some(objective.label)
        })
        .collect::<Vec<_>>();
    for (completed, objective) in progress.completed.iter_mut().zip(statuses) {
        *completed = objective.complete;
    }

    if progress.completed.iter().all(|complete| *complete) && !progress.won {
        progress.won = true;
        ProgressEvent::Victory
    } else if newly_completed.is_empty() {
        ProgressEvent::None
    } else {
        ProgressEvent::Completed(newly_completed)
    }
}

fn update_objective_progress(
    mut client: ResMut<ClientSimulation>,
    mut progress: ResMut<GameProgress>,
) {
    let event = apply_objective_progress(&mut progress, objective_statuses(&client));
    let attack_notification = take_attack_notification(&client, &mut progress);
    let notification = match event {
        ProgressEvent::None => attack_notification,
        ProgressEvent::Completed(labels) => Some(attack_notification.map_or_else(
            || format!("Objective complete: {}", labels.join(", ")),
            |attack| format!("{attack} — objective complete!"),
        )),
        ProgressEvent::Victory => {
            client.paused = true;
            Some("VICTORY — resilient solution online!".to_owned())
        }
    };
    if let Some(notification) = notification {
        progress.notification = Some(notification);
        progress.notification_timer.reset();
    }
}

fn take_attack_notification(
    client: &ClientSimulation,
    progress: &mut GameProgress,
) -> Option<String> {
    let report = client.last_report.as_ref()?;
    let attack = report.cyberattack?;
    if progress.last_announced_attack_tick == Some(report.tick.number()) {
        return None;
    }
    progress.last_announced_attack_tick = Some(report.tick.number());
    Some(if attack.blocked {
        format!(
            "CYBERATTACK BLOCKED — {} protected",
            service_description(&client.simulation, attack.target)
        )
    } else if report.failover_active {
        format!(
            "BREACH CONTAINED — failover served {} requests; loss {} credits",
            report.served, report.outage_penalty
        )
    } else {
        format!(
            "BREACH — {} disrupted for {} ticks",
            service_description(&client.simulation, attack.target),
            attack.disruption_ticks
        )
    })
}

fn update_notification(
    time: Res<Time>,
    mut progress: ResMut<GameProgress>,
    mut text: Single<&mut Text, With<NotificationText>>,
) {
    tick_notification(&mut progress, time.delta());
    **text = Text::new(progress.notification.clone().unwrap_or_default());
}

fn tick_notification(progress: &mut GameProgress, delta: Duration) {
    if progress.notification.is_some() && !progress.won {
        progress.notification_timer.tick(delta);
        if progress.notification_timer.just_finished() {
            progress.notification = None;
        }
    }
}

fn reset_scenario(
    client: &mut ClientSimulation,
    tool: &mut BuildTool,
    progress: &mut GameProgress,
) {
    client.simulation = create_new_game().expect("the new-game map dimensions must be valid");
    client.last_report = None;
    client.tick_timer = Timer::from_seconds(TICK_SECONDS, TimerMode::Repeating);
    client.paused = false;
    client.total_served = 0;
    client.blocked_attacks = 0;
    client.successful_failovers = 0;
    client.outage_losses = 0;
    client.capital_invested = 0;
    client.total_revenue = 0;
    client.operating_costs = 0;
    client.operating_cost_shortfall = 0;
    client.operating_profit = 0;

    tool.selected = ServiceKind::ApplicationServer;
    tool.hovered = None;
    tool.network_mode = None;
    tool.connection_from = None;
    tool.inspected = None;
    tool.feedback = "Scenario restarted; select with 1–4".to_owned();

    *progress = GameProgress::new();
    progress.notification = Some("Scenario restarted".to_owned());
}

fn visual_style(kind: ServiceKind) -> VisualStyle {
    match kind {
        ServiceKind::InternetGateway => VisualStyle {
            abbreviation: "GW",
            color: [0.15, 0.72, 0.92],
        },
        ServiceKind::Firewall => VisualStyle {
            abbreviation: "FW",
            color: [0.94, 0.55, 0.16],
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
        ServiceKind::Firewall => "Firewall",
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
        ServiceState::Upgrading { .. } => {
            let pulse = 0.65 + elapsed.sin().abs() * 0.35;
            Color::srgb(0.25, 0.7 * pulse, 1.0 * pulse)
        }
        ServiceState::Disrupted { .. } => {
            let pulse = 0.55 + elapsed.sin().abs() * 0.45;
            Color::srgb(0.95 * pulse, 0.08, 0.1)
        }
    }
}

fn scale_for_state(state: ServiceState, elapsed: f32) -> f32 {
    match state {
        ServiceState::Operational => 1.0,
        ServiceState::UnderConstruction { .. } => 0.94 + elapsed.sin().abs() * 0.06,
        ServiceState::Upgrading { .. } => 0.92 + elapsed.sin().abs() * 0.1,
        ServiceState::Disrupted { .. } => 0.9 + elapsed.sin().abs() * 0.1,
    }
}

const fn tier_ring_count(tier: ServiceTier) -> usize {
    match tier {
        ServiceTier::Starter => 0,
        ServiceTier::Scaled => 1,
        ServiceTier::Enterprise => 2,
    }
}

fn roi_percent(operating_profit: i128, capital_invested: u64) -> Option<f64> {
    if capital_invested == 0 {
        return None;
    }
    Some((operating_profit - i128::from(capital_invested)) as f64 / capital_invested as f64 * 100.0)
}

fn economics_text(client: &ClientSimulation) -> String {
    let roi = roi_percent(client.operating_profit, client.capital_invested)
        .map_or_else(|| "n/a".to_owned(), |roi| format!("{roi:.1}%"));
    format!(
        "ECONOMICS\n\nRevenue         {:>8}\nOperating costs {:>8}\nOutage losses   {:>8}\nOp. profit      {:>8}\nCapital invested{:>8}\nUnpaid costs    {:>8}\nROI             {:>8}",
        client.total_revenue,
        client.operating_costs,
        client.outage_losses,
        client.operating_profit,
        client.capital_invested,
        client.operating_cost_shortfall,
        roi,
    )
}

fn metrics_text(client: &ClientSimulation, tool: &BuildTool, status: &str) -> String {
    let simulation = &client.simulation;
    let report = client.last_report.as_ref();
    let demand = simulation.traffic().requests_per_tick();
    let served = report.map_or(0, |report| report.served);
    let dropped = report.map_or(0, |report| report.dropped);
    let mode = if let Some(network_mode) = tool.network_mode {
        let action = match network_mode {
            NetworkMode::Connect => "Connect",
            NetworkMode::Disconnect => "Disconnect",
        };
        tool.connection_from.map_or_else(
            || format!("Mode: {action} (choose source)"),
            |id| {
                format!(
                    "Mode: {action} from {}",
                    service_description(simulation, id)
                )
            },
        )
    } else {
        format!(
            "Mode: Build {} ({})",
            service_kind_name(tool.selected),
            tool.selected.build_cost()
        )
    };
    let inspection = tool
        .inspected
        .and_then(|id| inspection_text(client, id))
        .unwrap_or_else(|| "INSPECT\nRight-click a service".to_owned());
    let objectives = objectives_text(client);
    format!(
        "SERVUS  {status}\n\nTick         {:>6}\nCredits      {:>6}\nDemand       {:>6}\nServed       {:>6}\nDropped      {:>6}\nTotal served {:>6}\nAttacks held {:>6}\nFailovers    {:>6}\nOutage losses{:>6}\nThreat in    {:>6}\n\n{objectives}\n\nBUILD\n1  Gateway       50\n2  Load Balancer 75\n3  App Server   100\n4  Firewall     125\nC  Connection tool\nX  Disconnect tool\nU  Upgrade inspected\n- / +  Demand\n\n{mode}\n{}\n\n{inspection}\n\nSpace: pause / resume\nR: restart scenario",
        simulation.tick().number(),
        simulation.budget().credits(),
        demand,
        served,
        dropped,
        client.total_served,
        client.blocked_attacks,
        client.successful_failovers,
        client.outage_losses,
        ticks_until_attack(simulation.tick().number()),
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
    fn demand_adjustment_uses_fixed_steps_and_clamps_to_safe_bounds() {
        assert_eq!(adjusted_demand(100, true), 150);
        assert_eq!(adjusted_demand(100, false), 50);
        assert_eq!(adjusted_demand(0, false), 0);
        assert_eq!(adjusted_demand(MAX_DEMAND, true), MAX_DEMAND);
        assert_eq!(adjusted_demand(MAX_DEMAND - 20, true), MAX_DEMAND);
        assert_eq!(ticks_until_attack(0), CYBER_ATTACK_INTERVAL);
        assert_eq!(ticks_until_attack(7), 1);
        assert_eq!(ticks_until_attack(8), CYBER_ATTACK_INTERVAL);
    }

    #[test]
    fn traffic_markers_reflect_volume_and_advance_along_the_link() {
        let start = Vec2::new(0.0, 0.0);
        let end = Vec2::new(200.0, 0.0);
        let first = traffic_markers(start, end, 150, 0.0);
        let later = traffic_markers(start, end, 150, 0.5);
        assert_eq!(first.len(), 3);
        assert_eq!(later.len(), 3);
        assert_ne!(first, later);
        assert!(
            first
                .iter()
                .all(|marker| marker.x > 0.0 && marker.x < 200.0)
        );
        assert!(first.iter().all(|marker| marker.y == 0.0));
        assert!(traffic_markers(start, end, 0, 0.0).is_empty());
        assert!(traffic_markers(start, start, 100, 0.0).is_empty());
    }

    #[test]
    fn every_service_kind_has_a_distinct_visual_identity() {
        let gateway = visual_style(ServiceKind::InternetGateway);
        let firewall = visual_style(ServiceKind::Firewall);
        let load_balancer = visual_style(ServiceKind::LoadBalancer);
        let server = visual_style(ServiceKind::ApplicationServer);
        assert_eq!(gateway.abbreviation, "GW");
        assert_eq!(firewall.abbreviation, "FW");
        assert_eq!(load_balancer.abbreviation, "LB");
        assert_eq!(server.abbreviation, "APP");
        assert_ne!(gateway.color, load_balancer.color);
        assert_ne!(gateway.color, firewall.color);
        assert_ne!(firewall.color, load_balancer.color);
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
        let upgrading = ServiceState::Upgrading {
            target: ServiceTier::Scaled,
            ticks_remaining: 2,
        };
        assert_ne!(
            scale_for_state(upgrading, 0.0),
            scale_for_state(upgrading, 1.0)
        );
        assert_eq!(tier_ring_count(ServiceTier::Starter), 0);
        assert_eq!(tier_ring_count(ServiceTier::Scaled), 1);
        assert_eq!(tier_ring_count(ServiceTier::Enterprise), 2);
        let disrupted = ServiceState::Disrupted { ticks_remaining: 2 };
        assert_ne!(
            scale_for_state(disrupted, 0.0),
            scale_for_state(disrupted, 1.0)
        );
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
    fn upgrade_tool_starts_the_next_tier_and_reports_its_capital_cost() {
        let map = MapSize::new(2, 2).expect("test map is valid");
        let mut simulation = Simulation::new(300, 0, map);
        let server = build_service(
            &mut simulation,
            ServiceKind::ApplicationServer,
            GridPosition::new(0, 0),
        )
        .expect("server placement is valid");
        for _ in 0..ServiceKind::ApplicationServer.construction_ticks() {
            simulation.advance();
        }
        let credits_before = simulation.budget().credits();

        assert_eq!(
            try_upgrade_service(&mut simulation, server.id()),
            Ok(UpgradeAction {
                from: ServiceTier::Starter,
                to: ServiceTier::Scaled,
                cost: 80,
            })
        );
        assert_eq!(simulation.budget().credits(), credits_before - 80);
        assert_eq!(
            simulation
                .service(server.id())
                .map(|service| service.state()),
            Some(ServiceState::Upgrading {
                target: ServiceTier::Scaled,
                ticks_remaining: 2,
            })
        );
    }

    #[test]
    fn network_tools_create_and_remove_a_directed_link() {
        let map = MapSize::new(3, 3).expect("test map is valid");
        let mut simulation = Simulation::new(300, 0, map);
        let gateway = build_service(
            &mut simulation,
            ServiceKind::InternetGateway,
            GridPosition::new(0, 0),
        )
        .expect("gateway placement is valid");
        let server = build_service(
            &mut simulation,
            ServiceKind::ApplicationServer,
            GridPosition::new(1, 0),
        )
        .expect("server placement is valid");
        let mut from = None;

        assert_eq!(
            try_connection_click(
                &mut simulation,
                &mut from,
                gateway.position(),
                NetworkMode::Connect,
            ),
            Ok(ConnectionAction::SourceSelected(gateway.id()))
        );
        assert_eq!(from, Some(gateway.id()));
        assert!(can_connect(&simulation, gateway.id(), server.id()));
        assert_eq!(
            try_connection_click(
                &mut simulation,
                &mut from,
                server.position(),
                NetworkMode::Connect,
            ),
            Ok(ConnectionAction::Connected {
                from: gateway.id(),
                to: server.id(),
            })
        );
        assert_eq!(from, None);
        assert!(simulation.network().has_link(gateway.id(), server.id()));
        assert!(!simulation.network().has_link(server.id(), gateway.id()));
        assert_eq!(simulation.budget().credits(), 140);
        assert!(!can_connect(&simulation, gateway.id(), server.id()));

        assert!(can_disconnect(&simulation, gateway.id(), server.id()));
        assert!(!can_disconnect(&simulation, server.id(), gateway.id()));
        assert_eq!(
            try_connection_click(
                &mut simulation,
                &mut from,
                gateway.position(),
                NetworkMode::Disconnect,
            ),
            Ok(ConnectionAction::SourceSelected(gateway.id()))
        );
        assert_eq!(
            try_connection_click(
                &mut simulation,
                &mut from,
                server.position(),
                NetworkMode::Disconnect,
            ),
            Ok(ConnectionAction::Disconnected {
                from: gateway.id(),
                to: server.id(),
            })
        );
        assert!(!simulation.network().has_link(gateway.id(), server.id()));
        assert_eq!(simulation.budget().credits(), 140);
    }

    #[test]
    fn failed_disconnect_preserves_the_source_and_simulation() {
        let map = MapSize::new(2, 2).expect("test map is valid");
        let mut simulation = Simulation::new(150, 0, map);
        let gateway = build_service(
            &mut simulation,
            ServiceKind::InternetGateway,
            GridPosition::new(0, 0),
        )
        .expect("gateway placement is valid");
        let server = build_service(
            &mut simulation,
            ServiceKind::ApplicationServer,
            GridPosition::new(1, 0),
        )
        .expect("server placement is valid");
        let mut from = Some(gateway.id());
        let before = simulation.clone();

        let error = try_connection_click(
            &mut simulation,
            &mut from,
            server.position(),
            NetworkMode::Disconnect,
        )
        .expect_err("a missing directed link cannot be disconnected");
        assert_eq!(error.to_string(), "network link 1 -> 2 does not exist");
        assert_eq!(from, Some(gateway.id()));
        assert_eq!(simulation, before);
    }

    #[test]
    fn connection_tool_rejects_empty_tiles_without_losing_its_source() {
        let map = MapSize::new(3, 3).expect("test map is valid");
        let mut simulation = Simulation::new(100, 0, map);
        let gateway = build_service(
            &mut simulation,
            ServiceKind::InternetGateway,
            GridPosition::new(0, 0),
        )
        .expect("gateway placement is valid");
        let mut from = Some(gateway.id());
        let before = simulation.clone();
        let empty = GridPosition::new(2, 2);

        let error = try_connection_click(&mut simulation, &mut from, empty, NetworkMode::Connect)
            .expect_err("an empty tile is not a connection destination");
        assert_eq!(error, ConnectionClickError::EmptyTile(empty));
        assert_eq!(error.to_string(), "tile (2, 2) has no service");
        assert!(std::error::Error::source(&error).is_none());
        assert_eq!(from, Some(gateway.id()));
        assert_eq!(simulation, before);
    }

    #[test]
    fn failed_connection_preserves_the_source_and_simulation() {
        let map = MapSize::new(2, 2).expect("test map is valid");
        let mut simulation = Simulation::new(150, 0, map);
        let gateway = build_service(
            &mut simulation,
            ServiceKind::InternetGateway,
            GridPosition::new(0, 0),
        )
        .expect("gateway placement is valid");
        let server = build_service(
            &mut simulation,
            ServiceKind::ApplicationServer,
            GridPosition::new(1, 0),
        )
        .expect("server placement is valid");
        let mut from = Some(gateway.id());
        let before = simulation.clone();

        let error = try_connection_click(
            &mut simulation,
            &mut from,
            server.position(),
            NetworkMode::Connect,
        )
        .expect_err("no credits remain for the connection");
        assert_eq!(
            error.to_string(),
            "not enough credits: 10 required, 0 available"
        );
        assert!(std::error::Error::source(&error).is_some());
        assert_eq!(from, Some(gateway.id()));
        assert_eq!(simulation, before);
    }

    #[test]
    fn objectives_follow_infrastructure_routing_and_total_service() {
        let mut simulation = create_new_game().expect("new game is valid");
        let gateway = build_service(
            &mut simulation,
            ServiceKind::InternetGateway,
            GridPosition::new(0, 0),
        )
        .expect("gateway placement is valid");
        let firewall = build_service(
            &mut simulation,
            ServiceKind::Firewall,
            GridPosition::new(1, 0),
        )
        .expect("firewall placement is valid");
        let load_balancer = build_service(
            &mut simulation,
            ServiceKind::LoadBalancer,
            GridPosition::new(2, 0),
        )
        .expect("load-balancer placement is valid");
        let server = build_service(
            &mut simulation,
            ServiceKind::ApplicationServer,
            GridPosition::new(3, 0),
        )
        .expect("server placement is valid");
        simulation
            .apply(GameCommand::ConnectServices {
                from: gateway.id(),
                to: firewall.id(),
            })
            .expect("first test connection is affordable");
        simulation
            .apply(GameCommand::ConnectServices {
                from: firewall.id(),
                to: load_balancer.id(),
            })
            .expect("second test connection is affordable");
        simulation
            .apply(GameCommand::ConnectServices {
                from: load_balancer.id(),
                to: server.id(),
            })
            .expect("third test connection is affordable");

        let mut total_served = 0;
        let mut total_revenue = 0;
        let mut operating_costs = 0;
        let mut operating_profit = 0;
        for _ in 0..ServiceKind::ApplicationServer.construction_ticks() {
            let report = simulation.advance();
            total_served += report.served;
            total_revenue += report.revenue;
            operating_costs += report.operating_cost;
            operating_profit += report.net_income;
        }
        let mut client = ClientSimulation {
            simulation,
            last_report: None,
            tick_timer: Timer::from_seconds(1.0, TimerMode::Repeating),
            paused: false,
            total_served,
            blocked_attacks: 0,
            successful_failovers: 0,
            outage_losses: 0,
            capital_invested: 380,
            total_revenue,
            operating_costs,
            operating_cost_shortfall: 0,
            operating_profit,
        };
        let objectives = objective_statuses(&client);
        assert!(objectives[..5].iter().all(|objective| objective.complete));
        assert!(!objectives[5].complete);
        assert!(!objectives[6].complete);
        assert!(!objectives[7].complete);
        assert!(!objectives[8].complete);

        while client.simulation.tick().number() < CYBER_ATTACK_INTERVAL {
            let report = client.simulation.advance();
            client.total_served += report.served;
            client.total_revenue += report.revenue;
            client.operating_costs += report.operating_cost;
            client.operating_cost_shortfall += report.operating_cost_shortfall;
            client.operating_profit += report.net_income;
            if report.cyberattack.is_some_and(|attack| attack.blocked) {
                client.blocked_attacks += 1;
            }
            client.last_report = Some(report);
        }
        let before_upgrade = objective_statuses(&client);
        assert!(
            before_upgrade[..8]
                .iter()
                .all(|objective| objective.complete)
        );
        assert!(!before_upgrade[8].complete);
        assert!(objectives_text(&client).contains("[x] Serve 500 total requests"));
        let mut progress = GameProgress::new();
        let notification = take_attack_notification(&client, &mut progress)
            .expect("the attack on tick eight is announced");
        assert!(notification.contains("CYBERATTACK BLOCKED"));
        assert!(notification.contains("Application Server #4 protected"));
        assert_eq!(take_attack_notification(&client, &mut progress), None);

        let upgrade = try_upgrade_service(&mut client.simulation, server.id())
            .expect("the objective upgrade is affordable");
        client.capital_invested += upgrade.cost;
        for _ in 0..2 {
            let report = client.simulation.advance();
            client.total_served += report.served;
            client.total_revenue += report.revenue;
            client.operating_costs += report.operating_cost;
            client.operating_cost_shortfall += report.operating_cost_shortfall;
            client.operating_profit += report.net_income;
            client.last_report = Some(report);
        }
        assert!(
            objective_statuses(&client)
                .iter()
                .all(|objective| objective.complete)
        );
    }

    #[test]
    fn objective_progress_emits_each_completion_once_then_victory() {
        let mut progress = GameProgress::new();
        let partial = [
            ObjectiveStatus {
                label: "one",
                complete: true,
            },
            ObjectiveStatus {
                label: "two",
                complete: true,
            },
            ObjectiveStatus {
                label: "three",
                complete: true,
            },
            ObjectiveStatus {
                label: "four",
                complete: true,
            },
            ObjectiveStatus {
                label: "five",
                complete: true,
            },
            ObjectiveStatus {
                label: "six",
                complete: true,
            },
            ObjectiveStatus {
                label: "seven",
                complete: true,
            },
            ObjectiveStatus {
                label: "eight",
                complete: true,
            },
            ObjectiveStatus {
                label: "nine",
                complete: false,
            },
        ];
        assert_eq!(
            apply_objective_progress(&mut progress, partial),
            ProgressEvent::Completed(vec![
                "one", "two", "three", "four", "five", "six", "seven", "eight"
            ])
        );
        assert_eq!(
            apply_objective_progress(&mut progress, partial),
            ProgressEvent::None
        );

        let complete = partial.map(|mut objective| {
            objective.complete = true;
            objective
        });
        assert_eq!(
            apply_objective_progress(&mut progress, complete),
            ProgressEvent::Victory
        );
        assert!(progress.won);
        assert_eq!(
            apply_objective_progress(&mut progress, complete),
            ProgressEvent::None
        );
    }

    #[test]
    fn ordinary_notifications_expire_but_victory_persists() {
        let mut progress = GameProgress::new();
        progress.notification = Some("Objective complete".to_owned());
        tick_notification(&mut progress, Duration::from_secs(2));
        assert_eq!(progress.notification.as_deref(), Some("Objective complete"));
        tick_notification(&mut progress, Duration::from_secs(1));
        assert_eq!(progress.notification, None);

        progress.notification = Some("VICTORY".to_owned());
        progress.won = true;
        tick_notification(&mut progress, Duration::from_secs(30));
        assert_eq!(progress.notification.as_deref(), Some("VICTORY"));
    }

    #[test]
    fn breach_notification_reports_partial_failover_and_financial_loss() {
        let map = MapSize::new(4, 2).expect("test map is valid");
        let mut simulation = Simulation::new(500, 150, map);
        let gateway = build_service(
            &mut simulation,
            ServiceKind::InternetGateway,
            GridPosition::new(0, 0),
        )
        .expect("gateway placement is valid");
        let load_balancer = build_service(
            &mut simulation,
            ServiceKind::LoadBalancer,
            GridPosition::new(1, 0),
        )
        .expect("load-balancer placement is valid");
        let first_server = build_service(
            &mut simulation,
            ServiceKind::ApplicationServer,
            GridPosition::new(2, 0),
        )
        .expect("first server placement is valid");
        let second_server = build_service(
            &mut simulation,
            ServiceKind::ApplicationServer,
            GridPosition::new(3, 0),
        )
        .expect("second server placement is valid");
        for (from, to) in [
            (gateway.id(), load_balancer.id()),
            (load_balancer.id(), first_server.id()),
            (load_balancer.id(), second_server.id()),
        ] {
            simulation
                .apply(GameCommand::ConnectServices { from, to })
                .expect("test connection is valid");
        }
        let mut report = simulation.advance();
        while report.tick.number() < CYBER_ATTACK_INTERVAL {
            report = simulation.advance();
        }
        assert!(report.failover_active);
        assert_eq!(report.served, 100);
        assert_eq!(report.outage_penalty, 50);

        let client = ClientSimulation {
            simulation,
            last_report: Some(report),
            tick_timer: Timer::from_seconds(1.0, TimerMode::Repeating),
            paused: false,
            total_served: 0,
            blocked_attacks: 0,
            successful_failovers: 1,
            outage_losses: 50,
            capital_invested: 0,
            total_revenue: 0,
            operating_costs: 0,
            operating_cost_shortfall: 0,
            operating_profit: 0,
        };
        let mut progress = GameProgress::new();
        let notification =
            take_attack_notification(&client, &mut progress).expect("the breach is announced once");
        assert!(notification.contains("BREACH CONTAINED"));
        assert!(notification.contains("served 100 requests"));
        assert!(notification.contains("loss 50 credits"));
    }

    #[test]
    fn restart_restores_every_piece_of_scenario_state() {
        let scenario = create_demo_scenario().expect("demo scenario is valid");
        let inspected = scenario.simulation.services()[0].id();
        let mut client = ClientSimulation {
            simulation: scenario.simulation,
            last_report: Some(
                create_demo_scenario()
                    .expect("demo scenario is valid")
                    .simulation
                    .advance(),
            ),
            tick_timer: Timer::from_seconds(9.0, TimerMode::Repeating),
            paused: true,
            total_served: 900,
            blocked_attacks: 3,
            successful_failovers: 2,
            outage_losses: 400,
            capital_invested: 500,
            total_revenue: 1_000,
            operating_costs: 200,
            operating_cost_shortfall: 10,
            operating_profit: 400,
        };
        let mut tool = BuildTool {
            selected: ServiceKind::InternetGateway,
            hovered: Some(GridPosition::new(1, 1)),
            network_mode: Some(NetworkMode::Connect),
            connection_from: Some(inspected),
            inspected: Some(inspected),
            feedback: "Old state".to_owned(),
        };
        let mut progress = GameProgress::new();
        progress.completed = [true; OBJECTIVE_COUNT];
        progress.won = true;
        progress.notification = Some("VICTORY".to_owned());

        reset_scenario(&mut client, &mut tool, &mut progress);

        assert!(client.simulation.services().is_empty());
        assert_eq!(client.simulation.budget().credits(), 500);
        assert_eq!(client.simulation.traffic().requests_per_tick(), 100);
        assert_eq!(client.last_report, None);
        assert!(!client.paused);
        assert_eq!(client.total_served, 0);
        assert_eq!(client.blocked_attacks, 0);
        assert_eq!(client.successful_failovers, 0);
        assert_eq!(client.outage_losses, 0);
        assert_eq!(client.capital_invested, 0);
        assert_eq!(client.total_revenue, 0);
        assert_eq!(client.operating_costs, 0);
        assert_eq!(client.operating_cost_shortfall, 0);
        assert_eq!(client.operating_profit, 0);
        assert_eq!(tool.selected, ServiceKind::ApplicationServer);
        assert_eq!(tool.network_mode, None);
        assert_eq!(tool.connection_from, None);
        assert_eq!(tool.inspected, None);
        assert_eq!(progress.completed, [false; OBJECTIVE_COUNT]);
        assert!(!progress.won);
        assert_eq!(progress.notification.as_deref(), Some("Scenario restarted"));
    }

    #[test]
    fn metrics_include_the_initial_scenario_state() {
        let scenario = create_demo_scenario().expect("demo scenario is valid");
        let client = ClientSimulation {
            simulation: scenario.simulation,
            last_report: None,
            tick_timer: Timer::from_seconds(1.0, TimerMode::Repeating),
            paused: false,
            total_served: 0,
            blocked_attacks: 0,
            successful_failovers: 0,
            outage_losses: 0,
            capital_invested: 0,
            total_revenue: 0,
            operating_costs: 0,
            operating_cost_shortfall: 0,
            operating_profit: 0,
        };
        let tool = BuildTool {
            selected: ServiceKind::ApplicationServer,
            hovered: None,
            network_mode: None,
            connection_from: None,
            inspected: None,
            feedback: "Ready".to_owned(),
        };
        let text = metrics_text(&client, &tool, "RUNNING");
        assert!(text.contains("Tick              0"));
        assert!(text.contains("Credits          45"));
        assert!(text.contains("Demand          200"));
        assert!(text.contains("Total served      0"));
        assert!(text.contains("Mode: Build Application Server (100)"));
        assert!(text.contains("Ready"));
    }

    #[test]
    fn economics_report_distinguishes_profit_capital_and_roi() {
        assert_eq!(roi_percent(0, 0), None);
        assert_eq!(roi_percent(50, 100), Some(-50.0));
        assert_eq!(roi_percent(100, 100), Some(0.0));
        assert_eq!(roi_percent(150, 100), Some(50.0));

        let scenario = create_demo_scenario().expect("demo scenario is valid");
        let client = ClientSimulation {
            simulation: scenario.simulation,
            last_report: None,
            tick_timer: Timer::from_seconds(1.0, TimerMode::Repeating),
            paused: false,
            total_served: 0,
            blocked_attacks: 0,
            successful_failovers: 0,
            outage_losses: 25,
            capital_invested: 100,
            total_revenue: 250,
            operating_costs: 75,
            operating_cost_shortfall: 5,
            operating_profit: 150,
        };

        let text = economics_text(&client);
        assert!(text.contains("Revenue              250"));
        assert!(text.contains("Operating costs       75"));
        assert!(text.contains("Outage losses         25"));
        assert!(text.contains("Op. profit           150"));
        assert!(text.contains("Capital invested     100"));
        assert!(text.contains("Unpaid costs           5"));
        assert!(text.contains("ROI                50.0%"));
    }

    #[test]
    fn metrics_describe_the_selected_connection_source() {
        let scenario = create_demo_scenario().expect("demo scenario is valid");
        let source = scenario.simulation.services()[0].id();
        let client = ClientSimulation {
            simulation: scenario.simulation,
            last_report: None,
            tick_timer: Timer::from_seconds(1.0, TimerMode::Repeating),
            paused: false,
            total_served: 0,
            blocked_attacks: 0,
            successful_failovers: 0,
            outage_losses: 0,
            capital_invested: 0,
            total_revenue: 0,
            operating_costs: 0,
            operating_cost_shortfall: 0,
            operating_profit: 0,
        };
        let tool = BuildTool {
            selected: ServiceKind::ApplicationServer,
            hovered: None,
            network_mode: Some(NetworkMode::Connect),
            connection_from: Some(source),
            inspected: None,
            feedback: "Choose destination".to_owned(),
        };
        let text = metrics_text(&client, &tool, "RUNNING");
        assert!(text.contains("Mode: Connect from Internet Gateway #1"));
        assert!(text.contains("Choose destination"));
    }

    #[test]
    fn inspection_reports_service_capacity_links_and_exact_flow() {
        let mut scenario = create_demo_scenario().expect("demo scenario is valid");
        let mut report = scenario.simulation.advance();
        for _ in 1..ServiceKind::ApplicationServer.construction_ticks() {
            report = scenario.simulation.advance();
        }
        let inspected = scenario.simulation.services()[2].id();
        let client = ClientSimulation {
            simulation: scenario.simulation,
            last_report: Some(report),
            tick_timer: Timer::from_seconds(1.0, TimerMode::Repeating),
            paused: false,
            total_served: 150,
            blocked_attacks: 0,
            successful_failovers: 0,
            outage_losses: 0,
            capital_invested: 0,
            total_revenue: 0,
            operating_costs: 0,
            operating_cost_shortfall: 0,
            operating_profit: 0,
        };

        let text = inspection_text(&client, inspected).expect("the inspected service exists");
        assert!(text.contains("Application Server #3"));
        assert!(text.contains("Tier          Starter"));
        assert!(text.contains("State         operational"));
        assert!(text.contains("Capacity      100 req/tick"));
        assert!(text.contains("Run cost      8/tick"));
        assert!(text.contains("Upgrade       Scaled: 80c / 2 ticks / cap 225"));
        assert!(text.contains("Links in/out  1/0"));
        assert!(text.contains("Flow in/out   100/0"));
    }
}
