import AppKit

/// Placeholder host target so Xcode can embed `BraidTunnel.appex`.
/// Daily UI still runs via `nsh run`; this binary exists for signing/VPN capability.
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
print("BraidHost: embed target for Network Extension. Use nsh run for the UI.")
RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.1))
