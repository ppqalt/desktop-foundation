import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Services.Pam
import "components"
import "theme"

ShellRoot {
    id: root
    property string failure: ""
    property var responseSource: null
    property int clearEpoch: 0
    property bool authenticated: false
    property bool finishing: false
    Component.onCompleted: Quickshell.watchFiles = false
    function submit(source): void {
        if (root.finishing) return;
        root.failure = "";
        root.responseSource = source;
        if (auth.responseRequired) {
            auth.respond(source.takeResponse());
        } else if (!auth.active) {
            if (!auth.start()) { root.failure = "Authentication unavailable"; Qt.exit(1); }
        }
    }
    IpcHandler {
        target: "sessionLock"
        function ensureLocked(): string {
            // Lock-only IPC: a repeated shortcut can cancel dismissal but can
            // never authenticate or release the compositor lock.
            finish.stop(); quitLater.stop();
            auth.abort();
            root.clearEpoch++;
            root.responseSource = null;
            root.failure = "";
            root.finishing = false;
            root.authenticated = false;
            sessionLock.locked = true;
            return sessionLock.secure ? "secure" : "pending";
        }
    }
    PamContext {
        id: auth
        config: "desktop-foundation-lock"
        configDirectory: "/etc/pam.d"
        // The current process account is used; no username comes from IPC/input.
        onPamMessage: {
            if (responseRequired && root.responseSource?.hasResponse) {
                respond(root.responseSource.takeResponse());
            }
        }
        onError: error => {
            // Configuration/helper failure is not a bad password. Leave the
            // compositor locked and let the supervisor start stock swaylock.
            Qt.exit(1);
        }
        onCompleted: result => {
            root.clearEpoch++;
            root.responseSource = null;
            if (result === PamResult.Success && sessionLock.secure) {
                root.authenticated = true;
                root.finishing = true;
                finish.start();
            } else {
                root.failure = result === PamResult.Failed || result === PamResult.MaxTries
                    ? "Incorrect password" : "Authentication unavailable";
            }
        }
    }
    WlSessionLock {
        id: sessionLock
        locked: true
        onSecureChanged: if (secure) {
            console.log("DF_LOCK_SECURE");
            if (root.authenticated && root.finishing) finish.restart();
        }
        WlSessionLockSurface {
            id: surface
            color: Qt.rgba(Material.colors.surface.r, Material.colors.surface.g, Material.colors.surface.b, 1)
            LockContent {
                id: content
                anchors.fill: parent
                accountName: Quickshell.env("USER")
                errorText: root.failure
                busy: root.finishing || (auth.active && !auth.responseRequired)
                clearEpoch: root.clearEpoch
                prompt: auth.responseRequired && auth.responseVisible ? "Response" : "Password"
                opacity: root.finishing ? 0 : 1
                Behavior on opacity { NumberAnimation { duration: Material.motion.fastEffects } }
                onSubmitted: root.submit(content)
                onCancelled: {
                    auth.abort();
                    root.failure = "";
                    root.clearEpoch++;
                }
                onBusyChanged: if (!busy) focusPassword()
            }
        }
    }
    Timer {
        id: finish
        interval: Material.motion.fastEffects
        onTriggered: {
            // No close/IPC/timer can reach this branch without PAM success.
            if (!root.authenticated || !sessionLock.secure) return;
            console.log("DF_LOCK_AUTHENTICATED");
            sessionLock.locked = false;
            quitLater.start();
        }
    }
    Timer { id: quitLater; interval: 100; onTriggered: Qt.quit() }
}
