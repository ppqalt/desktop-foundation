.pragma library

// Horizontal adjustment is shared by single and range sliders. Keep vertical
// movement for focus navigation, with modified keys available to the parent.
function direction(event, mirrored) {
    if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))
        return 0;
    const key = event.key;
    const left = key === Qt.Key_Left || key === Qt.Key_A;
    const right = key === Qt.Key_Right || key === Qt.Key_D;
    return (left ? -1 : right ? 1 : 0) * (mirrored ? -1 : 1);
}
