use std::{fmt, marker::PhantomData};

pub trait Command<State>: Clone {
    fn apply(&self, state: &mut State);
    fn revert(&self, state: &mut State);
}

#[derive(Clone)]
pub enum HistoryAction<Command> {
    Undo(usize),
    Command(Command)
}

impl<T> fmt::Display for HistoryAction<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HistoryAction::Undo(n) => write!(f, "undo {}x", n),
            HistoryAction::Command(t) => write!(f, "{}", t),
        }
    }
}

impl<T> fmt::Debug for HistoryAction<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HistoryAction::Undo(n) => write!(f, "Undo({})", n),
            HistoryAction::Command(t) => write!(f, "{:?}", t),
        }
    }
}

pub struct History<Command, State> {
    stack: Vec<HistoryAction<Command>>,
    current: usize,
    phantom: PhantomData<State>,

    limit: usize,
    threshold_index: usize,
    threshold_span: usize,
}

impl<Command, State> Default for History<Command, State> {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl<Cmd, State> History<Cmd, State> {
    pub fn new(limit: usize) -> Self {
        History {
            stack: Vec::new(),
            current: 0,
            phantom: PhantomData,
            limit,
            threshold_index: usize::MAX,
            threshold_span: 0,
        }
    }

    pub fn limit(&self) -> usize {
        self.limit
    }
}

impl<Cmd, State> History<Cmd, State>
where Cmd: Command<State> {
    pub fn current(&self) -> usize {
        self.current
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&HistoryAction<Cmd>> {
        self.stack.get(index)
    }

    pub fn iter(&self) -> impl Iterator<Item = &HistoryAction<Cmd>> {
        self.stack.iter()
    }

    pub fn apply(&mut self, command: Cmd, state: &mut State) {
        let action = HistoryAction::Command(command);

        self.apply_action(&action, state, self.current);
        self.stack.push(action);
        self.current = self.stack.len();

        self.check_limit();
    }

    pub fn undo(&mut self, state: &mut State) {
        if let Some(HistoryAction::Undo(n)) = self.stack.last() {
            if *n >= self.limit {
                return;
            }
        }

        if self.current > 0 {
            let undo_idx = self.current - 1;
            let cmd_to_revert = self.stack[undo_idx].clone();
            self.revert_action(&cmd_to_revert, state, undo_idx);
            self.current -= 1;

            if let Some(HistoryAction::Undo(n)) = self.stack.last_mut() {
                *n += 1;
            } else {
                self.stack.push(HistoryAction::Undo(1));
            }
            
            let index = self.stack.len() - 1;
            let Some(&HistoryAction::Undo(n)) = self.stack.last() else { unreachable!() };

            if index - n < self.threshold_index.saturating_sub(self.threshold_span) {
                self.threshold_index = index;
                self.threshold_span = n;
            }
        }
    }

    fn check_limit(&mut self) {
        let n = self.len();
        if n <= self.limit {
            return;
        }

        let count = n - self.limit;
        let threshold_target = self.threshold_index.saturating_sub(self.threshold_span);

        if threshold_target < count {
            if self.threshold_index >= count {
                return;
            }

            for (i, action) in self.stack.iter().enumerate().skip(count) {
                if let HistoryAction::Undo(span) = action {
                    let target = i.saturating_sub(*span);
                    if target < count {
                        return;
                    }
                }
            }
        }

        self.stack.drain(0 .. count);
        self.current = self.current.saturating_sub(count);
        
        self.threshold_index = usize::MAX;
        self.threshold_span = 0;

        for (i, action) in self.stack.iter().enumerate() {
            if let HistoryAction::Undo(n) = action {
                if i.saturating_sub(*n) < self.threshold_index.saturating_sub(self.threshold_span) {
                    self.threshold_index = i;
                    self.threshold_span = *n;
                }
            }
        }
    }

    pub fn redo(&mut self, state: &mut State) {
        if self.current < self.stack.len() {
            let cmd = self.stack[self.current].clone();
            let current = self.current;
            self.apply_action(&cmd, state, current);
            self.current += 1;

            let mut pop = false;
            if let Some(HistoryAction::Undo(n)) = self.stack.last_mut() {
                *n = n.saturating_sub(1);
                if *n == 0 {
                    pop = true;
                }
            }
            if pop {
                self.stack.pop();
            }
        }
    }

    fn apply_action(&mut self, action: &HistoryAction<Cmd>, state: &mut State, current_index: usize) {
        match action {
            HistoryAction::Command(command) => {
                command.apply(state);
            }
            HistoryAction::Undo(n) => {
                // Apply Undo(n) means we undo n commands starting from the one before this Undo command.
                // The Undo command is at `current_index`.
                // We need to revert commands at indices: current_index - 1, current_index - 2, ... current_index - n.
                for i in 0..*n {
                    if let Some(target_idx) = current_index.checked_sub(1 + i) {
                        if let Some(command) = self.stack.get(target_idx).cloned() {
                            self.revert_action(&command, state, target_idx);
                        }
                    }
                }
            }
        }
    }

    fn revert_action(&mut self, action: &HistoryAction<Cmd>, state: &mut State, command_index: usize) {
        match action {
            HistoryAction::Command(command) => {
                command.revert(state);
            }
            HistoryAction::Undo(n) => {
                // Reverting an Undo(n) means Redoing the commands it undid.
                // The Undo command is at `self.current - 1`.
                // It covers commands from `(self.current - 1) - n` to `(self.current - 1) - 1`.
                // We redo them in forward order.
                let undo_cmd_index = command_index;
                let start_index = undo_cmd_index.saturating_sub(*n);
                for i in 0..*n {
                    let target_index = start_index + i;
                    if let Some(command) = self.stack.get(target_index).cloned() {
                        self.apply_action(&command, state, target_index);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

#[derive(Debug, Clone)]
    enum TextCommand {
        Insert(String),
    }

    impl Command<String> for TextCommand {
        fn apply(&self, context: &mut String) {
            match self {
                TextCommand::Insert(s) => context.push_str(s),
            }
        }

        fn revert(&self, context: &mut String) {
            match self {
                TextCommand::Insert(s) => {
                    let start = context.len().saturating_sub(s.len());
                    context.truncate(start);
                }
            }
        }
    }


    #[test]
    fn test_history() {
        let mut state = String::from("Hello");
        let mut history = History::default();

        // Apply a command
        history.apply(TextCommand::Insert(" World".to_string()), &mut state);
        assert_eq!(state, "Hello World");

        // Undo
        history.undo(&mut state);
        assert_eq!(state, "Hello");

        // Apply another command
        history.apply(TextCommand::Insert(" Bob".to_string()), &mut state);
        assert_eq!(state, "Hello Bob");

        // All previous states are accessible by undoing history.
        history.undo(&mut state);
        assert_eq!(state, "Hello");
        history.undo(&mut state); // undoes the first undo
        assert_eq!(state, "Hello World");
        history.undo(&mut state);
        assert_eq!(state, "Hello"); // initial state

        // Redoing the first command
        history.redo(&mut state);
        assert_eq!(state, "Hello World");
    }
}