use clap::Parser;

#[derive(Parser, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryOutput {
    Link,
    Id,
    Path,
    Both,
}
