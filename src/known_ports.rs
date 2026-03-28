pub struct KnownPort {
    pub port: u16,
    pub name: &'static str,
    pub description: &'static str,
}

pub fn macos_ports() -> &'static [KnownPort] {
    &[
        KnownPort { port: 3031, name: "Remote Apple Events", description: "AppleScript remote program linking" },
        KnownPort { port: 3283, name: "Apple Remote Desktop", description: "ARD reporting and management" },
        KnownPort { port: 3689, name: "Music Sharing", description: "DAAP media sharing (Music.app)" },
        KnownPort { port: 5000, name: "AirPlay Receiver", description: "ControlCenter AirPlay HTTP" },
        KnownPort { port: 5100, name: "Camera Sharing", description: "ControlCenter camera/scanner sharing" },
        KnownPort { port: 5223, name: "Apple Push (APNS)", description: "iMessage, FaceTime, iCloud push" },
        KnownPort { port: 5297, name: "Bonjour Messaging", description: "LAN peer-to-peer messaging" },
        KnownPort { port: 5900, name: "Screen Sharing", description: "VNC/Screen Sharing server" },
        KnownPort { port: 7000, name: "AirPlay Streaming", description: "ControlCenter AirPlay media" },
        KnownPort { port: 8770, name: "Handoff", description: "sharingd Handoff and Universal Clipboard" },
        KnownPort { port: 9100, name: "Network Printing", description: "CUPS direct printing (PCL)" },
    ]
}

/// Look up a known macOS port. Returns (name, description) if found.
pub fn lookup(port: u16) -> Option<&'static KnownPort> {
    if cfg!(target_os = "macos") {
        macos_ports().iter().find(|kp| kp.port == port)
    } else {
        None
    }
}
