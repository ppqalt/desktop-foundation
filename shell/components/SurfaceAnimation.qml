import QtQuick
import "../theme"

// Behaviors retarget from the current value; never restart from a fixed origin.
NumberAnimation {
    duration: Theme.timing.normal
    easing.type: Theme.easing
    onDurationChanged: {
        if (running && duration === 0)
            complete();
    }
}
