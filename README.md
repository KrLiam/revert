
# revert

An implementation of a linear history-based command pattern, inspired by the concepts described in [The GURQ](https://github.com/zaboople/klonk/blob/master/TheGURQ.md). Unlike traditional undo/redo systems that discard history when a new action is performed after an undo, revert treats every action (including undo itself) as a new entry in an append-only history stack.

## Features

- **Linear History**: Undo operations are commands that are pushed onto the stack, revoking history rather than discarding it. As a result, undo operations can also be undone.
- **Maximum undo limit**: Provides a mechanism to only discard commands if they are too old to be undone.
- **Command Pattern**: Extensible `Command` trait with `execute` and `revert` methods, allowing for arbitrary state modifications.
- **Generic States**: Commands can modify any mutable state type (a simple data structure or complex app state).
- **Undo/Redo Operations**: Built-in methods to traverse back and forth in the command history.
- **Introspection**: Inspect the history stack to visualize user actions (e.g., for a history log UI).

## Usage

### Define Your Commands

Implement the `Command` trait for your command type. This trait defines how a command modifies your state (`execute`) and how to reverse that modification (`revert`).

```rust
use revert::{Command, History};

#[derive(Clone, Debug)]
pub enum TextCommand {
    Insert(String),
    Erase(String),
}

impl Command<String> for TextCommand {
    fn execute(&self, text: &mut String) {
        match self {
            TextCommand::Insert(s) => text.push_str(s),
            TextCommand::Erase(s) => {
                let new_len = text.len().saturating_sub(s.len());
                text.truncate(new_len);
            }
        }
    }

    fn revert(&self, text: &mut String) {
        match self {
            TextCommand::Insert(s) => {
                let new_len = text.len().saturating_sub(s.len());
                text.truncate(new_len);
            }
            TextCommand::Erase(s) => {
                text.push_str(s);
            }
        }
    }
}
```

### Initialize History

Create a `History` instance and provide the state to work on. In this case, the state is a `String`.

```rust
fn main() {
    let mut state = String::from("Hello");
    let mut history = History::default();

    // 1. execute a command
    history.execute(TextCommand::Insert(" World".to_string()), &mut state);
    assert_eq!(state, "Hello World");

    // 2. Undo
    history.undo(&mut state);
    assert_eq!(state, "Hello");

    // 3. execute another command
    history.execute(TextCommand::Insert(" Bob".to_string()), &mut state);
    assert_eq!(state, "Hello Bob");

    // All previous states are accessible by undoing history.

    history.undo(&mut state); // undoes 3
    assert_eq!(state, "Hello");

    history.undo(&mut state); // undoes 2
    assert_eq!(state, "Hello World");

    history.undo(&mut state); // undoes 1 (initial state)
    assert_eq!(state, "Hello");

    // Redoing the first command
    history.redo(&mut state);
    assert_eq!(state, "Hello World");
}
```

## Example

Check `examples/demo.rs` for a complete interactive example using Bevy.

```sh
cargo run --example demo
```
