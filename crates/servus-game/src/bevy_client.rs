use bevy::prelude::*;
use bevy::window::{WindowPlugin, WindowResolution};
use servus_sim::{
    GridPosition, MapSize, ServiceId, ServiceKind, ServiceState, Simulation, TickReport,
};

use crate::create_demo_scenario;

const TILE_SIZE: f32 = 72.0;
const MAP_OFFSET_X: f32 = 120.0;
const SERVICE_SIZE: f32 = 52.0;
const TICK_SECONDS: f32 = 1.25;

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
                advance_simulation,
                update_service_visuals,
                update_metrics,
                draw_map,
            )
                .chain(),
        )
        .run();
}

fn setup(mut commands: Commands, client: Res<ClientSimulation>) {
    commands.spawn(Camera2d);

    let map_size = client.simulation.map().size();
    for service in client.simulation.services() {
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

    commands.spawn((
        Text::new(metrics_text(&client, "RUNNING")),
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
        Text::new("Gateway → Load Balancer → Application Servers"),
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

fn toggle_pause(keys: Res<ButtonInput<KeyCode>>, mut client: ResMut<ClientSimulation>) {
    if keys.just_pressed(KeyCode::Space) {
        client.paused = !client.paused;
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

fn update_metrics(client: Res<ClientSimulation>, mut text: Single<&mut Text, With<MetricsText>>) {
    let status = if client.paused { "PAUSED" } else { "RUNNING" };
    **text = Text::new(metrics_text(&client, status));
}

fn draw_map(mut gizmos: Gizmos, client: Res<ClientSimulation>) {
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

fn metrics_text(client: &ClientSimulation, status: &str) -> String {
    let simulation = &client.simulation;
    let report = client.last_report.as_ref();
    let received = report.map_or(0, |report| report.received);
    let served = report.map_or(0, |report| report.served);
    let dropped = report.map_or(0, |report| report.dropped);
    format!(
        "SERVUS  {status}\n\nTick       {:>6}\nCredits    {:>6}\nDemand     {:>6}\nServed     {:>6}\nDropped    {:>6}\n\nSpace: pause / resume",
        simulation.tick().number(),
        simulation.budget().credits(),
        received,
        served,
        dropped,
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
    fn metrics_include_the_initial_scenario_state() {
        let scenario = create_demo_scenario().expect("demo scenario is valid");
        let client = ClientSimulation {
            simulation: scenario.simulation,
            last_report: None,
            tick_timer: Timer::from_seconds(1.0, TimerMode::Repeating),
            paused: false,
        };
        let text = metrics_text(&client, "RUNNING");
        assert!(text.contains("Tick            0"));
        assert!(text.contains("Credits        45"));
        assert!(text.contains("Demand          0"));
    }
}
