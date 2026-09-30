import QtQuick
import Quickshell
import "../utils/Search.js" as Search

QtObject {
    readonly property var entries: DesktopEntries.applications.values
    function search(query: string): var {
        return Search.rank(entries, query);
    }
    function launch(entry): bool {
        if (!entry || entry.command.length === 0)
            return false;
        let command = entry.command.slice();
        if (entry.runInTerminal)
            command = ["kitty", "--"].concat(command);
        // Argument arrays preserve desktop-entry parsing; never evaluate search text.
        Quickshell.execDetached({
            command: [Quickshell.shellDir + "/../scripts/launch"].concat(command),
            workingDirectory: entry.workingDirectory
        });
        return true;
    }
}
