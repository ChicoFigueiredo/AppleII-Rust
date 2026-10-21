//! P23 (gate G0): teclas de atalho de disco. Oráculo: AppleWin a8f0d1d (Windows/WinFrame.cpp).
//! Resultados em `docs/conformance/results/p23-disco.csv`.

use apple2_rust_shell::actions::{action_for, Action};
use apple2_rust_shell::keymap::{HostKey, Mods};

const NONE: Mods = Mods {
    shift: false,
    ctrl: false,
    alt: false,
};
const CTRL: Mods = Mods {
    shift: false,
    ctrl: true,
    alt: false,
};

/// GH#273, #690: a troca de discos tem tecla (F5 = BTN_DRIVESWAP, WinFrame.cpp:1426-1440 e :2098-2103),
/// que funciona em tela cheia porque as teclas F1-F8 são tratadas no `WM_KEYUP`.
#[test]
fn gh273_gh690_f5_swaps_the_floppy_disks() {
    assert_eq!(action_for(HostKey::F(5), NONE), Some(Action::SwapDrives));
}

/// GH#1224: no AppleWin a8f0d1d F3/F4 e Ctrl+F3/F4 só atendem os drives do slot 6
/// (WinFrame.cpp:1426-1432); o port faz o mesmo (não há tecla para o slot 5).
#[test]
fn gh1224_disk_hotkeys_address_only_slot_6_drives() {
    assert_eq!(
        action_for(HostKey::F(3), NONE),
        Some(Action::DriveSelect(0))
    );
    assert_eq!(
        action_for(HostKey::F(4), NONE),
        Some(Action::DriveSelect(1))
    );
    assert_eq!(action_for(HostKey::F(3), CTRL), Some(Action::DriveMenu(0)));
    assert_eq!(action_for(HostKey::F(4), CTRL), Some(Action::DriveMenu(1)));
}
