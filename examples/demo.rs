use bevy::{prelude::*, text::FontSmoothing};
use std::fmt;

use revert::history::{Command, History, HistoryAction};


#[derive(Debug, Clone)]
pub enum TextCommand {
    Insert(String),
    Erase(String),
}

impl Command<String> for TextCommand {
    fn execute(&self, context: &mut String) {
        match self {
            TextCommand::Insert(s) => context.push_str(s),
            TextCommand::Erase(s) => {
                let start = context.len().saturating_sub(s.len());
                context.truncate(start);
            }
        }
    }

    fn revert(&self, context: &mut String) {
        match self {
            TextCommand::Insert(s) => {
                let start = context.len().saturating_sub(s.len());
                context.truncate(start);
            }
            TextCommand::Erase(s) => {
                context.push_str(s);
            }
        }
    }
}

impl fmt::Display for TextCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TextCommand::Insert(s) => write!(f, "insert {:?}", s),
            TextCommand::Erase(s) => write!(f, "erase {:?}", s),
        }
    }
}


#[derive(Resource)]
pub struct EditorContent(pub String);
impl Default for EditorContent {
    fn default() -> Self {
        EditorContent("".to_string())
    }
}

#[derive(Resource, DerefMut, Deref, Default)]
pub struct EditorHistory(pub History<TextCommand, String>);


#[derive(Component)]
struct ContentTextMarker;

#[derive(Component)]
struct HistoryTextMarker;

#[derive(Component)]
struct PressedKeysTextMarker;

#[derive(Resource, Default)]
struct HistoryGraphLayout {
    nodes: Vec<GraphNodeData>,
}

struct GraphNodeData {
    idx: usize,
    pos: Vec2,
    parent_idx: Option<usize>,
}

#[derive(Component)]
struct GraphTextMarker;


fn setup(mut commands: Commands) {
    // UI requires a camera to be rendered
    commands.spawn(Camera2d::default());
}

fn setup_ui(mut commands: Commands) {
    // Spawn a root node that fills the screen and centers its children
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|parent| {
            // Spawn the text
            parent.spawn((
                Node {
                    max_width: Val::Px(500.0),
                    ..default()
                },
                Text::new(""),
                TextFont {
                    font_size: 50.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                ContentTextMarker,
            ));

            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(10.0),
                    right: Val::Px(10.0),
                    ..default()
                },
                Text::new(""),
                TextFont {
                    font_size: 15.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                HistoryTextMarker,
            ));

            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(10.0),
                    left: Val::Px(10.0),
                    ..default()
                },
                Text::new(""),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                PressedKeysTextMarker,
            ));
        });
}

fn insert_text(
    mut history: ResMut<EditorHistory>,
    mut content: ResMut<EditorContent>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    for key in keyboard.get_just_pressed() {
        // ignore if its ctrl
        if *key == KeyCode::ControlLeft || *key == KeyCode::ControlRight {
            continue;
        }
        // ignore if its z and ctrl is pressed
        if *key == KeyCode::KeyZ && keyboard.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) {
            continue;
        }

        match key {
            KeyCode::Space => {
                history.execute(TextCommand::Insert(" ".to_string()), &mut content.0);
            },
            _ => {
                let s = format!("{:?}", key);
                if let Some(char_part) = s.strip_prefix("Key") {
                    history.execute(TextCommand::Insert(char_part.to_lowercase()), &mut content.0);
                } else if let Some(digit_part) = s.strip_prefix("Digit") {
                    history.execute(TextCommand::Insert(digit_part.to_string()), &mut content.0);
                }
            }
        }
    }
}

fn erase_text(mut history: ResMut<EditorHistory>, mut content: ResMut<EditorContent>, input: Res<ButtonInput<KeyCode>>) {
    if input.just_pressed(KeyCode::Backspace) {
        if let Some(last_char) = content.0.chars().last() {
            history.execute(TextCommand::Erase(last_char.to_string()), &mut content.0);
        }
    }
}

fn pressed_undo(
    mut history: ResMut<EditorHistory>,
    mut content: ResMut<EditorContent>,
    input: Res<ButtonInput<KeyCode>>,
) {
    if input.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight])
        && !input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])
        && input.just_pressed(KeyCode::KeyZ)
    {
        if input.pressed(KeyCode::Tab) {
            let current = history.current();
            let mut target_idx = None;
            for i in (0..current).rev() {
                if let Some(HistoryAction::Undo(_)) = history.get(i) {
                    target_idx = Some(i);
                    break;
                }
            }

            if let Some(idx) = target_idx {
                let count = current - idx;
                if count <= history.limit() {
                    for _ in 0..count {
                        history.undo(&mut content.0);
                    }
                }
            }
        } else {
            history.undo(&mut content.0);
        }
    }
}

fn pressed_redo(
    mut history: ResMut<EditorHistory>,
    mut content: ResMut<EditorContent>,
    input: Res<ButtonInput<KeyCode>>,
) {
    if input.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight])
        && input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])
        && input.just_pressed(KeyCode::KeyZ)
    {
        history.redo(&mut content.0);
    }
}

fn update_content_text(
    content: Res<EditorContent>,
    mut query: Query<&mut Text, With<ContentTextMarker>>,
) {
    if content.is_changed() {
        for mut text in &mut query {
            text.0 = content.0.clone();
        }
    }
}

fn update_history_text(
    history: Res<EditorHistory>,
    mut query: Query<&mut Text, With<HistoryTextMarker>>,
) {
    if history.is_changed() {
        let mut text = "History\n".to_owned();
        for i in (0..history.len()).rev() {
            let prefix = if i + 1 == history.current() { "> " } else { "  " };
            let Some(action) = history.get(i) else { continue; };
            text.push_str(&format!("{} {}. {}\n", prefix, i, action));
        }
        if history.current() == 0 {
            text.push_str("> \n");
        }
        for mut t in &mut query {
            t.0 = text.clone();
        }
    }
}

fn update_pressed_keys_text(
    input: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut Text, With<PressedKeysTextMarker>>,
) {
    let mut parts = Vec::new();
    if input.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) {
        parts.push("ctrl");
    }
    if input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
        parts.push("shift");
    }
    if input.pressed(KeyCode::Tab) {
        parts.push("TAB");
    }
    if input.pressed(KeyCode::KeyZ) {
        parts.push("z");
    }

    let text = parts.join(" + ");
    for mut t in &mut query {
        t.0 = text.clone();
    }
}

fn update_graph_layout(
    history: Res<EditorHistory>,
    mut layout: ResMut<HistoryGraphLayout>,
    mut commands: Commands,
    text_query: Query<Entity, With<GraphTextMarker>>,
    window_query: Query<&Window>,
) {
    if !history.is_changed() {
        return;
    }

    for entity in &text_query {
        commands.entity(entity).despawn();
    }
    layout.nodes.clear();

    let Ok(window) = window_query.single() else { return };
    let start_pos = Vec2::new(30.0 - window.width() / 2.0, -50.0 + window.height() / 2.0);
    let spacing_x = 30.0;
    let spacing_y = 30.0;

    let mut current_idx = if history.current() > 0 {
        Some(history.current() - 1)
    } else {
        None
    };

    if let Some(mut idx) = current_idx {
        while let Some(HistoryAction::Undo(n)) = history.get(idx) {
            if idx < n + 1 {
                current_idx = None;
                break;
            }
            idx = idx - n - 1;
            current_idx = Some(idx);
        }
    }

    let mut positions: Vec<(i32, i32)> = Vec::new();

    for (i, action) in history.iter().enumerate() {
        let (grid_x, grid_y, parent_idx) = if i == 0 {
            (0, 0, None)
        } else {
            let prev_action = history.get(i - 1).unwrap();
            match prev_action {
                HistoryAction::Command(_) => {
                    let (px, py) = positions[i - 1];
                    (px + 1, py, Some(i - 1))
                }
                HistoryAction::Undo(n) => {
                    let mut cursor = (i - 1) as isize;
                    let mut current_n = *n;
                    
                    loop {
                        cursor = cursor - (current_n as isize) - 1;
                        if cursor < 0 {
                            break;
                        }
                        if let Some(HistoryAction::Undo(k)) = history.get(cursor as usize) {
                            current_n = *k;
                        } else {
                            break;
                        }
                    }

                    let parent = if cursor >= 0 { Some(cursor as usize) } else { None };
                    
                    let (px, _py) = if let Some(pidx) = parent {
                        positions[pidx]
                    } else {
                        (-1, 0)
                    };
                    
                    let prev_y = positions[i-1].1;
                    (px + 1, prev_y + 1, parent)
                }
            }
        };

        positions.push((grid_x, grid_y));

        let pos = start_pos + Vec2::new(grid_x as f32 * spacing_x, -(grid_y as f32) * spacing_y);

        let label = match action {
            HistoryAction::Command(cmd) => match cmd {
                TextCommand::Insert(s) => format!("+{}", s),
                TextCommand::Erase(s) => format!("-{}", s),
            },
            HistoryAction::Undo(_) => continue,
        };

        layout.nodes.push(GraphNodeData { idx: i, pos, parent_idx });

        let bg_color = if Some(i) == current_idx {
            Color::linear_rgb(0.5, 1.0, 0.5)
        } else {
            Color::WHITE
        };

        commands.spawn((
            Sprite {
                color: bg_color,
                custom_size: Some(Vec2::new(20.0, 20.0)),
                ..default()
            },
            Transform::from_translation(pos.extend(1.0)),
            GraphTextMarker,
        )).with_children(|parent| {
            parent.spawn((
                Text2d::new(label),
                TextFont {
                    font_size: 10.0,
                    weight: FontWeight(1000),
                    font_smoothing: FontSmoothing::None,
                    ..default()
                },
                TextColor(Color::BLACK),
                Transform::from_translation(Vec3::Z),
            ));
        });
    }
}

fn draw_graph_gizmos(layout: Res<HistoryGraphLayout>, mut gizmos: Gizmos) {
    for node in &layout.nodes {
        if let Some(pidx) = node.parent_idx {
            if let Some(parent_node) = layout.nodes.iter().find(|n| n.idx == pidx) {
                gizmos.line_2d(parent_node.pos + vec2(12.0, 0.0), node.pos - vec2(12.0, 0.0), Color::WHITE);
            }
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_resource::<EditorContent>()
        .init_resource::<HistoryGraphLayout>()
        .insert_resource(EditorHistory(History::new(1000)))
        .add_systems(Startup, (setup, setup_ui))
        .add_systems(PreUpdate, (insert_text, erase_text, pressed_undo, pressed_redo))
        .add_systems(FixedUpdate, update_content_text)
        .add_systems(Update, (update_history_text, update_pressed_keys_text, update_graph_layout, draw_graph_gizmos))
        .run();
}

#[cfg(test)]
mod tests {
    use revert::history::History;

    use crate::TextCommand;

    #[test]
    fn it_works() {
        let mut state = String::from("Hello");
        let mut history = History::default();

        history.execute(TextCommand::Insert(" World".to_string()), &mut state);
        assert_eq!(state, "Hello World");
        
        history.undo(&mut state);
        assert_eq!(state, "Hello");
        
        history.execute(TextCommand::Insert(" Dave".to_string()), &mut state);
        assert_eq!(state, "Hello Dave");
        
        history.undo(&mut state);
        history.undo(&mut state);
        assert_eq!(state, "Hello World");
    }
}
