import QtQuick

// One visible/input-owning modal. Retiring backend owners can finish offscreen.
QtObject {
    id: root
    required property var surfaces
    property string pending: ""
    function request(name, showOnly = false) {
        const target = surfaces[name];
        if (!target)
            return;
        // Non-modal sidebars coexist with popups and with each other. Only an
        // explicit request for that sidebar changes its lifecycle.
        if (target.independent) {
            const sidebar = target.item();
            if (target.external)
                target.open(false);
            else if (!sidebar && target.opening?.() && !showOnly)
                target.dismiss();
            else if (sidebar && !sidebar.closing && !showOnly)
                target.dismiss();
            else if (sidebar)
                target.present();
            else
                target.open(false);
            return;
        }
        for (const key of Object.keys(surfaces)) {
            if (key === "power" && surfaces[key].item()?.busy)
                return;
        }
        if (pending === name && !showOnly) {
            pending = "";
            return;
        }
        pending = "";
        const item = target.item();
        const same = item && !item.superseded;
        let replacing = false;
        // Remove the outgoing window from rendering/input immediately, without
        // destroying a Process that still owns a clipboard/device operation.
        for (const key of Object.keys(surfaces)) {
            if (surfaces[key].independent)
                continue;
            if (key !== name && surfaces[key].external)
                surfaces[key].dismiss();
            const outgoing = surfaces[key].item();
            if (key !== name && outgoing) {
                replacing = replacing || !outgoing.superseded;
                outgoing.superseded = true;
                if (!outgoing.closing)
                    surfaces[key].dismiss();
            }
        }
        if (target.external) {
            target.open(false);
            return;
        }
        if (target.retiring?.()) {
            pending = name;
            return;
        }
        if (item) {
            if (same && !item.closing && !showOnly)
                target.dismiss();
            else {
                item.handoff = replacing;
                item.superseded = false;
                target.present();
            }
        } else
            target.open(replacing);
    }
    function drain(): void {
        for (const key of Object.keys(surfaces)) {
            const item = surfaces[key].item();
            if (item?.superseded && !item.closing)
                surfaces[key].dismiss();
        }
        // Only a requested target's own irreversible shutdown can delay reuse.
        if (pending && !surfaces[pending].alive()) {
            const next = pending;
            pending = "";
            surfaces[next].open(true);
        }
    }
    readonly property bool retiring: Object.keys(surfaces).some(key => !!surfaces[key].item()?.superseded)
    property Timer retirement: Timer {
        interval: 16
        repeat: true
        running: root.pending !== "" || root.retiring
        onTriggered: root.drain()
    }
}
