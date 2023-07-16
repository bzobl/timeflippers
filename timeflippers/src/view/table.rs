use std::fmt;

pub enum Position {
    Top,
    Center,
    Bottom,
}

impl Position {
    fn left(&self) -> &'static str {
        use Position::*;
        match self {
            Top => "┌",
            Center => "├",
            Bottom => "└",
        }
    }

    fn separator(&self) -> &'static str {
        use Position::*;
        match self {
            Top => "┬",
            Center => "┼",
            Bottom => "┴",
        }
    }

    fn right(&self) -> &'static str {
        use Position::*;
        match self {
            Top => "┐",
            Center => "┤",
            Bottom => "┘",
        }
    }
}

pub struct TableHeader<'a> {
    pub columns: Vec<(&'a str, usize)>,
    pub position: Position,
}

impl<'a> fmt::Display for TableHeader<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}─", self.position.left())?;
        for (i, (name, width)) in self.columns.iter().enumerate() {
            write!(
                f,
                "{}{:─^width$}",
                if i == 0 {
                    ""
                } else {
                    self.position.separator()
                },
                name,
                width = width
            )?;
        }
        write!(f, "─{}", self.position.right())
    }
}
