import QtQuick

// Router adapter for an externally rendered surface. Native code owns its state,
// motion and shutdown; this adapter only serializes finite lifecycle requests.
QtObject {
    id: root
    required property list<string> commandPrefix
    property string toggleAction: "toggle"
    property list<string> queue: []
    readonly property bool busy: worker.running || queue.length > 0
    signal failed(string message)
    function send(action: string): void {
        let next = queue.slice();
        if (action === "close")
            next = [action];
        else if (action === "toggle" && next[next.length - 1] === "toggle") {
            // Two queued toggles cancel, preserving the latest intent.
            next.pop();
        } else
            next.push(action);
        queue = next;
        pump();
    }
    function pump(): void {
        if (worker.running || !queue.length)
            return;
        const action = queue[0];
        queue = queue.slice(1);
        worker.command = commandPrefix.concat([action === "toggle" ? toggleAction : action]);
        worker.running = true;
    }
    property ControlRequest worker: ControlRequest {
        expectJson: false
        onFinished: code => {
            if (code !== 0)
                root.failed(failure);
            Qt.callLater(root.pump);
        }
    }
}
