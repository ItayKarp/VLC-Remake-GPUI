//! The tray menu's commands and the ids its items carry.

use super::*;

#[test]
fn menu_ids_name_their_commands() {
    for command in TrayCommand::ALL {
        assert_ne!(command.id(), 0);
        assert_eq!(TrayCommand::from_id(i64::from(command.id())), Some(command));
    }
    assert_eq!(TrayCommand::from_id(0), None);
    assert_eq!(TrayCommand::from_id(6), None);
    assert_eq!(TrayCommand::from_id(-1), None);
}

#[test]
fn the_menu_holds_every_command_once() {
    let listed: Vec<_> = TrayCommand::MENU.iter().flat_map(|group| group.iter()).collect();
    assert_eq!(listed.len(), TrayCommand::ALL.len());
    assert!(TrayCommand::ALL.iter().all(|command| listed.contains(&command)));
}
