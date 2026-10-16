// Legacy 1.0 scanner is retained only as a regression reference. Desktop commands
// exclusively use the reviewed inventory pipeline; old permanent-delete commands
// are not registered or compiled into the application.
#[cfg(test)]
#[allow(dead_code)]
mod cleaner;
pub mod inventory;

#[cfg(test)]
mod cleaner_tests;
