import QtQuick
import "../theme"

// Effects never overshoot a color role. Match shared opacity effects.
ColorAnimation {
    duration: Material.motion.fastEffects
    easing.type: Easing.BezierSpline
    easing.bezierCurve: Material.effectsCurve
    onDurationChanged: {
        if (running && duration === 0)
            complete();
    }
}
