use std::{fmt, marker::PhantomData};

/// A command that can be executed on a state.
pub trait Command<State> {
    /// Executes the command on the given state.
    fn execute(&self, state: &mut State);

    /// Reverts the command on the given state.
    fn revert(&self, state: &mut State);

    /// Merges the command with another command.
    ///
    /// Returns `true` if the command was merged, `false` otherwise.
    #[allow(unused)]
    fn merge(&mut self, other: &Self) -> bool {
        false
    }
}

/// An action in a history stack. Can either be a command or an undo.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum HistoryAction<Command> {
    Undo(usize),
    Command(Command)
}
impl<Command> HistoryAction<Command> {
    /// Tries to unwrap the action as an undo action.
    pub fn as_undo(&self) -> Option<usize> {
        match self {
            HistoryAction::Undo(n) => Some(*n),
            _=> None
        }
    }
    
    /// Tries to unwrap the action as a command.
    pub fn as_command(&self) -> Option<&Command> {
        match self {
            HistoryAction::Command(c) => Some(c),
            _=> None
        }
    }

    /// Tries to mutably unwrap the action as a command.
    pub fn as_command_mut(&mut self) -> Option<&mut Command> {
        match self {
            HistoryAction::Command(c) => Some(c),
            _=> None
        }
    }
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

/// A command history stack.
pub struct History<Command, State> {
    stack: Vec<HistoryAction<Command>>,
    next: usize,
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
    /// Creates a new history stack with the given limit.
    pub fn new(limit: usize) -> Self {
        History {
            stack: Vec::new(),
            next: 0,
            limit,
            threshold_index: usize::MAX,
            threshold_span: 0,
            phantom: PhantomData,
        }
    }

    /// Returns the limit of the history stack.
    pub fn limit(&self) -> usize {
        self.limit
    }
}

impl<Cmd, State> History<Cmd, State>
where Cmd: Command<State> {
    /// Returns the index of the current (last executed) action. Might be
    /// `None` if no actions were executed yet.
    pub fn current_idx(&self) -> Option<usize> {
        self.next.checked_sub(1)
    }
    
    /// Returns the index of the current (last executed) action.
    pub fn next_idx(&self) -> usize {
        self.next
    }

    // Returns the number of actions currently in the history stack.
    pub fn len(&self) -> usize {
        self.stack.len()
    }
    
    /// Returns whether the history stack is empty.
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// Returns a reference to the action at the given index.
    pub fn get(&self, index: usize) -> Option<&HistoryAction<Cmd>> {
        self.stack.get(index)
    }
    
    /// Returns a reference to the last executed action.
    pub fn get_next_undo(&self) -> Option<&HistoryAction<Cmd>> {
        self.next.checked_sub(1).and_then(|i| self.stack.get(i))
    }

    /// Returns a reference to the next action that can be redone (the last undone action).
    pub fn get_next_redo(&self) -> Option<&HistoryAction<Cmd>> {
        self.next.checked_add(1).and_then(|i| self.stack.get(i))
    }

    /// Returns whether there is an action that can be undone.
    pub fn can_undo(&self) -> bool {
        self.get_next_undo().is_some()
    }

    /// Returns whether there is an action that can be redone.
    pub fn can_redo(&self) -> bool {
        self.get_next_redo().is_some()
    }

    /// Returns an iterator for the history stack.
    pub fn iter(&self) -> impl Iterator<Item = &HistoryAction<Cmd>> {
        self.stack.iter()
    }

    /// Clears the history stack resets it to its initial state.
    pub fn clear(&mut self) {
        self.stack.clear();
        self.next = 0;
        self.threshold_index = usize::MAX;
        self.threshold_span = 0;
    }

    /// Append a command to the history stack without executing it.
    pub fn append(&mut self, command: Cmd) -> usize {
        let last_command = self.stack.last_mut().and_then(|a| a.as_command_mut());
        if let Some(last_action) = last_command {
            if last_action.merge(&command) {
                return self.stack.len() - 1;
            }
        }

        self.stack.push(HistoryAction::Command(command));
        self.next = self.stack.len();

        self.check_limit();

        self.stack.len() - 1
    }

    /// Append a command to the history stack and execute it.
    pub fn execute(&mut self, command: Cmd, state: &mut State) {
        command.execute(state);
        self.append(command);
    }

    /// Undo the last executed command.
    pub fn undo(&mut self, state: &mut State) {
        if let Some(HistoryAction::Undo(n)) = self.stack.last() {
            if *n >= self.limit {
                return;
            }
        }

        if self.next > 0 {
            let undo_idx = self.next - 1;
            self.revert_action(undo_idx, state);
            self.next -= 1;

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

    /// Redoes the last undone command.
    pub fn redo(&mut self, state: &mut State) {
        if self.next < self.stack.len() {
            self.execute_action(self.next, state);
            self.next += 1;

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

    fn execute_action(&mut self, action_idx: usize, state: &mut State) {
        let Some(action) = self.stack.get(action_idx) else { return };

        match action {
            HistoryAction::Command(command) => {
                command.execute(state);
            }
            HistoryAction::Undo(n) => {
                // We need to revert commands at indices: action_idx - 1, action_idx - 2, ... action_idx - n.
                for i in 0..*n {
                    if let Some(target_idx) = action_idx.checked_sub(1 + i) {
                        self.revert_action(target_idx, state);
                    }
                }
            }
        }
    }

    fn revert_action(&mut self, action_idx: usize, state: &mut State) {
        let Some(action) = self.stack.get(action_idx) else { return };

        match action {
            HistoryAction::Command(command) => {
                command.revert(state);
            }
            HistoryAction::Undo(n) => {
                // Reverting an Undo(n) means Redoing the commands it undid.
                // The Undo command is at `self.next - 1`.
                // It covers commands from `(self.next - 1) - n` to `(self.next - 1) - 1`.
                // We redo them in forward order.
                let start_index = action_idx.saturating_sub(*n);
                for i in 0..*n {
                    let target_index = start_index + i;
                    self.execute_action(target_index, state);
                }
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
        self.next = self.next.saturating_sub(count);
        
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
}

#[cfg(test)]
mod tests {
    use super::*;

#[derive(Debug, Clone)]
    enum TextCommand {
        Insert(String),
    }

    impl Command<String> for TextCommand {
        fn execute(&self, context: &mut String) {
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
        history.execute(TextCommand::Insert(" World".to_string()), &mut state);
        assert_eq!(state, "Hello World");

        // Undo
        history.undo(&mut state);
        assert_eq!(state, "Hello");

        // Apply another command
        history.execute(TextCommand::Insert(" Bob".to_string()), &mut state);
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