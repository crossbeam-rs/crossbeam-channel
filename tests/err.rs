extern crate crossbeam_channel;
use std::sync::mpsc;
use crossbeam_channel as cc;

fn send_into() {
    {
        let std = mpsc::TrySendError::Full("full");
        let cross = cc::TrySendError::Full("full");

        assert_eq!(cross, std.into());
        assert_eq!(std, cross.into());
        assert_eq!(cross, cc::TrySendError::from(std));

        // This is the only one that _doesn't_ work
        // assert_eq!(std, mpsc::TrySendError::from(cross));
    }
    {
        let std = mpsc::TrySendError::Disconnected("dis");
        let cross = cc::TrySendError::Disconnected("dis");

        assert_eq!(cross, std.into());
        assert_eq!(std, cross.into());
        assert_eq!(cross, cc::TrySendError::from(std));

        // This is the only one that _doesn't_ work
        // assert_eq!(std, mpsc::TrySendError::from(cross));
    }
}

fn recv_into() {
    {
        let std = mpsc::TryRecvError::Empty;
        let cross = cc::TryRecvError::Empty;

        assert_eq!(cross, std.into());
        assert_eq!(std, cross.into());
        assert_eq!(cross, cc::TryRecvError::from(std));

        // This is the only one that _doesn't_ work
        // assert_eq!(std, mpsc::TrySendError::from(cross));
    }
    {
        let std = mpsc::TryRecvError::Disconnected;
        let cross = cc::TryRecvError::Disconnected;

        assert_eq!(cross, std.into());
        assert_eq!(std, cross.into());
        assert_eq!(cross, cc::TryRecvError::from(std));

        // This is the only one that _doesn't_ work
        // assert_eq!(std, mpsc::TrySendError::from(cross));
    }
}
