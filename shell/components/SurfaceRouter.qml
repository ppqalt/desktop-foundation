import QtQuick

// Last intent wins. A new modal never maps over an outgoing modal.
QtObject {
    id: root
    required property var surfaces
    property string pending: ""
    function request(name, showOnly = false) {
        const target = surfaces[name];
        if (!target)
            return;
        if (pending === name && !showOnly) {
            pending = "";
            return;
        }
        // An in-flight system action cannot be dismissed or replaced.
        for (const key of Object.keys(surfaces)) {
            if (surfaces[key].item()?.busy && key === "power")
                return;
        }
        if (target.retiring?.()) {
            pending = name;
            return;
        }
        const item = target.item();
        if (item) {
            pending = "";
            if (item.closing || showOnly)
                target.present();
            else
                target.dismiss();
            return;
        }
        pending = name;
        for (const key of Object.keys(surfaces)) {
            if (surfaces[key].item())
                surfaces[key].dismiss();
        }
        drain();
    }
    function drain(): void {
        if (!pending)
            return;
        for (const key of Object.keys(surfaces)) {
            if (surfaces[key].alive()) {
                // A closing surface already owns its completion. Retargeting
                // it every frame restarts sidebar motion indefinitely.
                if (!surfaces[key].item()?.closing)
                    surfaces[key].dismiss();
                return;
            }
        }
        const next = pending;
        pending = "";
        surfaces[next].open();
    }
    // Runs only while a different surface is finishing its exit/owned request.
    property Timer handoff: Timer {
        interval: 16
        repeat: true
        running: root.pending !== ""
        onTriggered: root.drain()
    }
}
