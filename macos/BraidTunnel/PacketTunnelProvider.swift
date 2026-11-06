import Foundation
import NetworkExtension
import os.log

/// Packet Tunnel provider for Braid (P3b-NE).
///
/// Reads sing-box JSON from the App Group and runs the pinned `sing-box`
/// binary with that config. The host app writes the file before starting
/// the VPN via `NETunnelProviderManager`.
final class PacketTunnelProvider: NEPacketTunnelProvider {
    private let log = OSLog(subsystem: "com.chaxus.braid.tunnel", category: "tunnel")
    private var singBox: Process?

    private static let appGroupId = "group.com.chaxus.braid"
    private static let configRelativePath = "Library/Application Support/Braid/ne-config.json"
    private static let statusRelativePath = "Library/Application Support/Braid/ne-status.json"

    override func startTunnel(options: [String: NSObject]?, completionHandler: @escaping (Error?) -> Void) {
        os_log("startTunnel", log: log, type: .info)
        writeStatus(state: "starting", detail: nil)

        guard let container = FileManager.default.containerURL(
            forSecurityApplicationGroupIdentifier: Self.appGroupId
        ) else {
            let err = TunnelError.appGroupUnavailable
            writeStatus(state: "failed", detail: err.localizedDescription)
            completionHandler(err)
            return
        }

        let configURL = container.appendingPathComponent(Self.configRelativePath)
        guard FileManager.default.fileExists(atPath: configURL.path) else {
            let err = TunnelError.configMissing(configURL.path)
            writeStatus(state: "failed", detail: err.localizedDescription)
            completionHandler(err)
            return
        }

        // Minimal settings: sing-box owns the real utun when run with TUN inbound.
        // System VPN still needs a settings object so the session can start.
        let settings = NEPacketTunnelNetworkSettings(tunnelRemoteAddress: "127.0.0.1")
        settings.mtu = 1500
        let ipv4 = NEIPv4Settings(addresses: ["10.255.0.1"], subnetMasks: ["255.255.255.0"])
        ipv4.includedRoutes = [NEIPv4Route.default()]
        settings.ipv4Settings = ipv4
        settings.dnsSettings = NEDNSSettings(servers: ["1.1.1.1"])

        setTunnelNetworkSettings(settings) { [weak self] error in
            guard let self else { return }
            if let error {
                self.writeStatus(state: "failed", detail: error.localizedDescription)
                completionHandler(error)
                return
            }
            do {
                try self.spawnSingBox(configURL: configURL, container: container)
                self.writeStatus(state: "up", detail: nil)
                completionHandler(nil)
            } catch {
                self.writeStatus(state: "failed", detail: error.localizedDescription)
                completionHandler(error)
            }
        }
    }

    override func stopTunnel(with reason: NEProviderStopReason, completionHandler: @escaping () -> Void) {
        os_log("stopTunnel reason=%{public}d", log: log, type: .info, reason.rawValue)
        terminateSingBox()
        writeStatus(state: "down", detail: nil)
        completionHandler()
    }

    private func spawnSingBox(configURL: URL, container: URL) throws {
        let binary = try resolveSingBoxBinary(container: container)
        let process = Process()
        process.executableURL = binary
        process.arguments = ["run", "-c", configURL.path]
        let logFile = container
            .appendingPathComponent("Library/Application Support/Braid/sing-box.ne.log")
        try FileManager.default.createDirectory(
            at: logFile.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        FileManager.default.createFile(atPath: logFile.path, contents: nil)
        let handle = try FileHandle(forWritingTo: logFile)
        process.standardOutput = handle
        process.standardError = handle
        process.standardInput = nil
        try process.run()
        singBox = process

        // Brief settle — if it exits immediately, surface failure.
        Thread.sleep(forTimeInterval: 0.4)
        if !process.isRunning {
            throw TunnelError.singBoxExitedEarly
        }
    }

    private func resolveSingBoxBinary(container: URL) throws -> URL {
        let candidates = [
            container.appendingPathComponent("Library/Application Support/Braid/bin/sing-box"),
            Bundle.main.url(forAuxiliaryExecutable: "sing-box"),
            Bundle.main.url(forResource: "sing-box", withExtension: nil),
        ].compactMap { $0 }

        for url in candidates {
            if FileManager.default.isExecutableFile(atPath: url.path) {
                return url
            }
        }
        throw TunnelError.singBoxMissing
    }

    private func terminateSingBox() {
        guard let process = singBox else { return }
        process.terminate()
        process.waitUntilExit()
        singBox = nil
    }

    private func writeStatus(state: String, detail: String?) {
        guard let container = FileManager.default.containerURL(
            forSecurityApplicationGroupIdentifier: Self.appGroupId
        ) else { return }
        let url = container.appendingPathComponent(Self.statusRelativePath)
        try? FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        var payload: [String: Any] = [
            "state": state,
            "ts": Int(Date().timeIntervalSince1970),
        ]
        if let detail { payload["detail"] = detail }
        guard let data = try? JSONSerialization.data(withJSONObject: payload, options: [.prettyPrinted]) else {
            return
        }
        try? data.write(to: url, options: .atomic)
    }
}

enum TunnelError: LocalizedError {
    case appGroupUnavailable
    case configMissing(String)
    case singBoxMissing
    case singBoxExitedEarly

    var errorDescription: String? {
        switch self {
        case .appGroupUnavailable:
            return "App Group group.com.chaxus.braid is unavailable"
        case .configMissing(let path):
            return "NE config missing at \(path)"
        case .singBoxMissing:
            return "sing-box binary not found in App Group or appex bundle"
        case .singBoxExitedEarly:
            return "sing-box exited immediately after start"
        }
    }
}
