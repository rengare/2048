use bevy_easings::*;
use std::{cmp::Ordering, collections::HashMap, ops::Range};

use bevy::prelude::*;
use itertools::Itertools;
use rand::prelude::*;

mod colors;
mod ui;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::hex("#1f2638").unwrap()))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "2048".to_string(),
                ..default()
            }),
            ..default()
        }))
        .add_plugin(EasingsPlugin)
        .add_state::<GameState>()
        .add_plugin(ui::GameUIPlugin)
        .init_resource::<FontSpec>()
        .init_resource::<Game>()
        .add_event::<NewTileEvent>()
        .add_systems((game_reset, spawn_tiles).in_schedule(OnEnter(GameState::Playing)))
        .add_startup_systems((setup, spawn_board, apply_system_buffers).chain())
        .add_systems(
            (
                render_tile_points,
                board_shift,
                render_tiles,
                new_tile_handler,
                end_game,
            )
                .in_set(OnUpdate(GameState::Playing)),
        )
        .run()
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash, States)]
enum GameState {
    #[default]
    Playing,
    GameOver,
}

struct NewTileEvent;

#[derive(Default, Resource)]
struct Game {
    score: u32,
    best_score: u32,
}

const TILE_SIZE: f32 = 80.0;
const TILE_PADDING: f32 = 10.0;

#[derive(Debug, Component, PartialEq)]
struct Points {
    value: u32,
}

#[derive(Component)]
struct TileText;

#[derive(Resource)]
struct FontSpec {
    family: Handle<Font>,
}

impl FromWorld for FontSpec {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.get_resource::<AssetServer>().unwrap();
        let font = asset_server.load("fonts/FiraSans-Bold.ttf");

        Self { family: font }
    }
}

#[derive(Debug, Component, PartialEq, Eq, Hash)]
struct Position {
    x: u8,
    y: u8,
}

#[derive(Component)]
struct Board {
    size: u8,
    physical_size: f32,
}

impl Board {
    fn new(size: u8) -> Self {
        Self {
            size,
            physical_size: f32::from(size) * TILE_SIZE + f32::from(size + 1) * TILE_PADDING,
        }
    }

    fn cell_position_to_physical(&self, pos: u8) -> f32 {
        let offset = -self.physical_size / 2.0 + TILE_SIZE / 2.0;

        offset + f32::from(pos) * TILE_SIZE + f32::from(pos + 1) * TILE_PADDING
    }

    fn to_vec2(&self) -> Vec2 {
        Vec2::new(self.physical_size, self.physical_size)
    }
}

fn spawn_board(mut commands: Commands) {
    let board = Board::new(4);

    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: colors::BOARD,
                custom_size: Some(board.to_vec2()),
                ..default()
            },
            ..default()
        })
        .with_children(|builder| {
            for tile in (0..board.size).cartesian_product(0..board.size) {
                let sprite = Sprite {
                    color: colors::TILE_PLACEHOLDER,
                    custom_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
                    ..default()
                };

                builder.spawn(SpriteBundle {
                    sprite,
                    transform: Transform::from_xyz(
                        board.cell_position_to_physical(tile.0),
                        board.cell_position_to_physical(tile.1),
                        1.0,
                    ),
                    ..default()
                });
            }
        })
        .insert(board);
}

fn spawn_tiles(mut commands: Commands, query_board: Query<&Board>, font_spec: Res<FontSpec>) {
    let board = query_board.single();

    let mut rng = rand::thread_rng();

    let starting_tiles: Vec<(u8, u8)> = (0..board.size)
        .cartesian_product(0..board.size)
        .choose_multiple(&mut rng, 2);

    for (x, y) in starting_tiles.iter() {
        let pos = Position { x: *x, y: *y };
        spawn_tile(&mut commands, board, &font_spec, pos);
    }
}

fn spawn_tile(commands: &mut Commands, board: &Board, font_spec: &Res<FontSpec>, pos: Position) {
    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: colors::TILE,
                custom_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
                ..default()
            },
            transform: Transform::from_xyz(
                board.cell_position_to_physical(pos.x),
                board.cell_position_to_physical(pos.y),
                1.0,
            ),
            ..default()
        })
        .with_children(|builder| {
            let text_bundle: Text2dBundle = Text2dBundle {
                text: Text::from_section(
                    "2",
                    TextStyle {
                        font: font_spec.family.clone(),
                        font_size: 40.0,
                        color: Color::BLACK,
                        ..default()
                    },
                )
                .with_alignment(TextAlignment::Center),
                transform: Transform::from_xyz(0.0, 0.0, 1.0),
                ..default()
            };

            builder.spawn(text_bundle).insert(TileText);
        })
        .insert(Points { value: 2 })
        .insert(pos);
}

fn render_tile_points(
    mut texts: Query<&mut Text, With<TileText>>,
    tiles: Query<(&Points, &Children)>,
) {
    for (points, children) in tiles.iter() {
        if let Some(entity) = children.first() {
            let mut text = texts.get_mut(*entity).expect("expected Text to exist");
            let text_section = text
                .sections
                .first_mut()
                .expect("expected TextSection to exist");
            text_section.value = points.value.to_string();
        };
    }
}

enum BoardShift {
    Left,
    Right,
    Up,
    Down,
}

impl BoardShift {
    fn sort(&self, a: &Position, b: &Position) -> Ordering {
        match self {
            BoardShift::Left => match Ord::cmp(&a.y, &b.y) {
                Ordering::Equal => Ord::cmp(&a.x, &b.x),
                ord => ord,
            },
            BoardShift::Right => match Ord::cmp(&b.y, &a.y) {
                Ordering::Equal => Ord::cmp(&b.x, &a.x),
                ord => ord,
            },
            BoardShift::Up => match Ord::cmp(&b.x, &a.x) {
                Ordering::Equal => Ord::cmp(&b.y, &a.y),
                ord => ord,
            },
            BoardShift::Down => match Ord::cmp(&a.x, &b.x) {
                Ordering::Equal => Ord::cmp(&a.y, &b.y),
                ord => ord,
            },
        }
    }

    fn set_column_position(&self, board_size: u8, pos: &mut Mut<Position>, col: u8) {
        match self {
            BoardShift::Left => pos.x = col,
            BoardShift::Right => pos.x = board_size - 1 - col,
            BoardShift::Up => pos.y = board_size - 1 - col,
            BoardShift::Down => pos.y = col,
        }
    }

    fn get_row_position(&self, pos: &Position) -> u8 {
        match self {
            BoardShift::Left | BoardShift::Right => pos.y,
            BoardShift::Up | BoardShift::Down => pos.x,
        }
    }
}

impl TryFrom<&KeyCode> for BoardShift {
    type Error = &'static str;

    fn try_from(value: &KeyCode) -> Result<Self, Self::Error> {
        match value {
            KeyCode::Left => Ok(Self::Left),
            KeyCode::Right => Ok(Self::Right),
            KeyCode::Up => Ok(Self::Up),
            KeyCode::Down => Ok(Self::Down),
            _ => Err("not valid key"),
        }
    }
}

fn board_shift(
    mut commands: Commands,
    input: Res<Input<KeyCode>>,
    board: Query<&Board>,
    mut tiles: Query<(Entity, &mut Position, &mut Points)>,
    mut new_tile_events: EventWriter<NewTileEvent>,
    mut game: ResMut<Game>,
) {
    let board = board.single();

    let direction = input
        .get_just_pressed()
        .find_map(|key| BoardShift::try_from(key).ok());

    if let Some(board_shift) = direction {
        let mut it = tiles
            .iter_mut()
            .sorted_by(|a, b| board_shift.sort(&a.1, &b.1))
            .peekable();

        let mut col: u8 = 0;

        while let Some(mut tile) = it.next() {
            board_shift.set_column_position(board.size, &mut tile.1, col);

            if let Some(next_tile) = it.peek() {
                if board_shift.get_row_position(&next_tile.1)
                    != board_shift.get_row_position(&tile.1)
                {
                    col = 0;
                } else if next_tile.2.value != tile.2.value {
                    col += 1;
                } else {
                    // merge
                    let real_next_tile = it.next().expect("expected next tile");
                    tile.2.value *= 2;
                    game.score += tile.2.value;

                    commands.entity(real_next_tile.0).despawn_recursive();

                    if let Some(future_tile) = it.peek() {
                        if board_shift.get_row_position(&future_tile.1)
                            != board_shift.get_row_position(&tile.1)
                        {
                            col = 0;
                        } else {
                            col += 1;
                        }
                    }
                }
            }
        }
        new_tile_events.send(NewTileEvent);

        if game.best_score < game.score {
            game.best_score = game.score;
        }
    }
}

fn render_tiles(
    mut commands: Commands,
    mut tiles: Query<(Entity, &mut Transform, &Position, Changed<Position>)>,
    query_board: Query<&Board>,
) {
    let board = query_board.single();

    for (entity, transform, pos, pos_changed) in tiles.iter_mut() {
        if pos_changed {
            let x = board.cell_position_to_physical(pos.x);
            let y = board.cell_position_to_physical(pos.y);
            commands.entity(entity).insert(transform.ease_to(
                Transform::from_xyz(x, y, transform.translation.z),
                EaseFunction::QuadraticInOut,
                EasingType::Once {
                    duration: std::time::Duration::from_millis(100),
                },
            ));
        }
    }
}

fn new_tile_handler(
    mut tile_reader: EventReader<NewTileEvent>,
    mut commands: Commands,
    query_board: Query<&Board>,
    tiles: Query<&Position>,
    font_spec: Res<FontSpec>,
) {
    let board = query_board.single();

    for _event in tile_reader.iter() {
        let mut rng = rand::thread_rng();

        let possible_position: Option<Position> = (0..board.size)
            .cartesian_product(0..board.size)
            .filter_map(|tile_pos| {
                let new_pos = Position {
                    x: tile_pos.0,
                    y: tile_pos.1,
                };

                match tiles.iter().find(|pos| **pos == new_pos) {
                    Some(_) => None,
                    None => Some(new_pos),
                }
            })
            .choose(&mut rng);

        if let Some(pos) = possible_position {
            spawn_tile(&mut commands, board, &font_spec, pos);
        }
    }
}

fn end_game(
    tiles: Query<(&Position, &Points)>,
    query_board: Query<&Board>,
    mut run_state: ResMut<NextState<GameState>>,
) {
    let board = query_board.single();

    let max_tiles = 16;

    if tiles.iter().len() == max_tiles {
        let map: HashMap<&Position, &Points> = tiles.iter().collect();
        let neighbor_points = [(-1, 0), (0, 1), (1, 0), (0, -1)];
        let board_range: Range<i8> = 0..(board.size as i8);

        let has_move = tiles.iter().any(|(Position { x, y }, value)| {
            neighbor_points
                .iter()
                .filter_map(|(x2, y2)| {
                    let new_x = *x as i8 - x2;
                    let new_y = *y as i8 - y2;

                    if !board_range.contains(&new_x) || !board_range.contains(&new_y) {
                        return None;
                    };

                    map.get(&Position {
                        x: new_x.try_into().unwrap(),
                        y: new_y.try_into().unwrap(),
                    })
                })
                .any(|&v| v == value)
        });

        if !has_move {
            dbg!("game over!");
            run_state.set(GameState::GameOver);
        }
    }
}

fn game_reset(
    mut commands: Commands,
    tiles: Query<Entity, With<Position>>,
    mut game: ResMut<Game>,
) {
    for entity in tiles.iter() {
        commands.entity(entity).despawn_recursive();
    }

    game.score = 0;
}
